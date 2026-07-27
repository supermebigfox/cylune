#import "bridge.h"
#import "pet_lifecycle.h"

#import <CoreGraphics/CoreGraphics.h>
#import <CoreMedia/CoreMedia.h>
#import <CoreVideo/CoreVideo.h>
#import <IOSurface/IOSurface.h>
#import <ScreenCaptureKit/ScreenCaptureKit.h>
#import <dispatch/dispatch.h>
#import <os/lock.h>

static const uint32_t kPetCallbackPermissionChanged = 7;
static const uint32_t kPetCallbackCaptureFailed = 8;
static_assert(PetSafeCapturePolicy().maximum_retained_frames == 1,
              "capture owns exactly one newest IOSurface");

static const char *CaptureStateName(uint32_t state) {
  switch (state) {
    case PET_CAPTURE_UNAVAILABLE:
      return "unavailable";
    case PET_CAPTURE_NOT_DETERMINED:
      return "not_determined";
    case PET_CAPTURE_DENIED:
      return "denied";
    case PET_CAPTURE_RESTART_REQUIRED:
      return "restart_required";
    case PET_CAPTURE_READY:
      return "ready";
    case PET_CAPTURE_FAILED:
      return "failed";
    default:
      return "failed";
  }
}

@protocol BPCaptureControlling <NSObject>
- (void)configureRegion:(PetCaptureRegion)region
               realMode:(BOOL)realMode
                visible:(BOOL)visible
      requestPermission:(BOOL)requestPermission
                    fps:(uint32_t)fps;
- (void)stop;
- (void)shutdown;
- (uint32_t)captureState;
- (IOSurfaceRef)copyLatestSurface CF_RETURNS_RETAINED;
@end

@interface BPUnavailableCaptureService : NSObject <BPCaptureControlling>
- (instancetype)initWithCallback:(PetCallback)callback;
@end

@implementation BPUnavailableCaptureService {
  PetCallback _callback;
}

- (instancetype)initWithCallback:(PetCallback)callback {
  self = [super init];
  if (self) {
    _callback = callback;
  }
  return self;
}

- (void)configureRegion:(PetCaptureRegion)region
               realMode:(BOOL)realMode
                visible:(BOOL)visible
      requestPermission:(BOOL)requestPermission
                    fps:(uint32_t)fps {
  (void)region;
  (void)visible;
  (void)requestPermission;
  (void)fps;
  if (realMode && _callback != nullptr) {
    _callback(kPetCallbackPermissionChanged,
              CaptureStateName(PET_CAPTURE_UNAVAILABLE), 0.0, 0.0, 0);
  }
}

- (void)stop {}

- (void)shutdown {
  _callback = nullptr;
}

- (uint32_t)captureState {
  return PET_CAPTURE_UNAVAILABLE;
}

- (IOSurfaceRef)copyLatestSurface {
  return nullptr;
}

@end

API_AVAILABLE(macos(12.3))
@interface BPScreenCaptureService
    : NSObject <BPCaptureControlling, SCStreamOutput, SCStreamDelegate>
- (instancetype)initWithCallback:(PetCallback)callback;
@end

@implementation BPScreenCaptureService {
  PetCallback _callback;
  SCStream *_stream;
  dispatch_queue_t _frameQueue;
  os_unfair_lock _surfaceLock;
  IOSurfaceRef _latestSurface;
  void *_activeStreamIdentity;
  PetFrameRetention _frameRetention;
  NSUInteger _generation;
  uint32_t _captureState;
  PetPermissionLifecycle _permission;
  BOOL _shuttingDown;
}

- (instancetype)initWithCallback:(PetCallback)callback {
  self = [super init];
  if (self) {
    _callback = callback;
    _frameQueue =
        dispatch_queue_create("com.local.bambuspools.capture.frames",
                              DISPATCH_QUEUE_SERIAL);
    _surfaceLock = OS_UNFAIR_LOCK_INIT;
    _captureState = PET_CAPTURE_UNAVAILABLE;
  }
  return self;
}

- (void)dealloc {
  [self releaseLatestSurface];
}

- (void)setCaptureState:(uint32_t)state {
  if (_captureState == state) {
    return;
  }
  _captureState = state;
  if (_callback != nullptr) {
    _callback(kPetCallbackPermissionChanged, CaptureStateName(state), 0.0,
              0.0, 0);
  }
}

- (void)failCapture {
  [self stopStreamAndReleaseFrame];
  [self setCaptureState:PET_CAPTURE_FAILED];
  if (_callback != nullptr) {
    _callback(kPetCallbackCaptureFailed, "capture_failed", 0.0, 0.0, 0);
  }
}

- (void)configureRegion:(PetCaptureRegion)region
               realMode:(BOOL)realMode
                visible:(BOOL)visible
      requestPermission:(BOOL)requestPermission
                    fps:(uint32_t)fps {
  if (_shuttingDown) {
    return;
  }
  if (!realMode) {
    [self stopStreamAndReleaseFrame];
    [self setCaptureState:PET_CAPTURE_UNAVAILABLE];
    return;
  }
  if (!visible) {
    [self stopStreamAndReleaseFrame];
    return;
  }
  if (region.source_width <= 0.0 || region.source_height <= 0.0 ||
      region.pixel_width == 0 || region.pixel_height == 0) {
    [self failCapture];
    return;
  }

  if (@available(macOS 12.3, *)) {
    const BOOL preflightGranted = CGPreflightScreenCaptureAccess();
    const PetPermissionDecision decision =
        _permission.preflight(preflightGranted, requestPermission);
    [self setCaptureState:decision.state];
    if (decision.action == PetPermissionAction::kRequestSystemPermission) {
      [self stopStreamAndReleaseFrame];
      const BOOL granted = CGRequestScreenCaptureAccess();
      const PetPermissionDecision result =
          _permission.request_result(granted);
      [self setCaptureState:result.state];
      return;
    }
    if (decision.action != PetPermissionAction::kEnumerateCapture) {
      [self stopStreamAndReleaseFrame];
      return;
    }

    [self enumerateAndStartRegion:region fps:fps];
  } else {
    [self stopStreamAndReleaseFrame];
    [self setCaptureState:PET_CAPTURE_UNAVAILABLE];
  }
}

- (void)enumerateAndStartRegion:(PetCaptureRegion)region fps:(uint32_t)fps {
  if (@available(macOS 12.3, *)) {
    [self stopStreamAndReleaseFrame];
    const NSUInteger generation = _generation;
    __weak BPScreenCaptureService *weakSelf = self;
    [SCShareableContent
        getShareableContentExcludingDesktopWindows:NO
                               onScreenWindowsOnly:NO
                                  completionHandler:^(
                                      SCShareableContent *content,
                                      NSError *error) {
      dispatch_async(dispatch_get_main_queue(), ^{
        BPScreenCaptureService *strongSelf = weakSelf;
        if (strongSelf == nil || strongSelf->_shuttingDown ||
            generation != strongSelf->_generation) {
          return;
        }
        if (error != nil || content == nil) {
          [strongSelf failCapture];
          return;
        }
        [strongSelf startWithContent:content region:region fps:fps
                         generation:generation];
      });
    }];
  }
}

- (void)startWithContent:(SCShareableContent *)content
                  region:(PetCaptureRegion)region
                     fps:(uint32_t)fps
              generation:(NSUInteger)generation API_AVAILABLE(macos(12.3)) {
  if (@available(macOS 12.3, *)) {
    const PetCapturePolicy policy = PetSafeCapturePolicy();
    SCDisplay *selectedDisplay = nil;
    for (SCDisplay *display in content.displays) {
      if (display.displayID == region.display_id) {
        selectedDisplay = display;
        break;
      }
    }

    SCRunningApplication *ownApplication = nil;
    const pid_t ownProcess = NSProcessInfo.processInfo.processIdentifier;
    for (SCRunningApplication *application in content.applications) {
      if (application.processID == ownProcess) {
        ownApplication = application;
        break;
      }
    }
    if (selectedDisplay == nil ||
        (policy.excludes_own_process && ownApplication == nil)) {
      [self failCapture];
      return;
    }

    NSArray<SCRunningApplication *> *excludedApplications =
        policy.excludes_own_process ? @[ ownApplication ] : @[];
    SCContentFilter *filter = [[SCContentFilter alloc]
             initWithDisplay:selectedDisplay
        excludingApplications:excludedApplications
             exceptingWindows:@[]];
    SCStreamConfiguration *configuration =
        [[SCStreamConfiguration alloc] init];
    configuration.sourceRect =
        CGRectMake(region.source_x, region.source_y, region.source_width,
                   region.source_height);
    configuration.width = region.pixel_width;
    configuration.height = region.pixel_height;
    configuration.pixelFormat = kCVPixelFormatType_32BGRA;
    configuration.queueDepth = policy.queue_depth;
    configuration.showsCursor = policy.shows_cursor;
    configuration.minimumFrameInterval =
        CMTimeMake(1, fps == 60 ? 60 : 30);
    if (@available(macOS 13.0, *)) {
      configuration.capturesAudio = policy.captures_audio;
      configuration.excludesCurrentProcessAudio = YES;
    }
    if (@available(macOS 15.0, *)) {
      configuration.captureMicrophone = policy.captures_microphone;
    }

    SCStream *stream = [[SCStream alloc] initWithFilter:filter
                                          configuration:configuration
                                               delegate:self];
    NSError *outputError = nil;
    if (![stream addStreamOutput:self
                            type:SCStreamOutputTypeScreen
              sampleHandlerQueue:_frameQueue
                           error:&outputError] ||
        outputError != nil) {
      [self failCapture];
      return;
    }

    _stream = stream;
    os_unfair_lock_lock(&_surfaceLock);
    _activeStreamIdentity = (__bridge void *)stream;
    _frameRetention.start();
    os_unfair_lock_unlock(&_surfaceLock);
    __weak BPScreenCaptureService *weakSelf = self;
    [stream startCaptureWithCompletionHandler:^(NSError *startError) {
      dispatch_async(dispatch_get_main_queue(), ^{
        BPScreenCaptureService *strongSelf = weakSelf;
        if (strongSelf == nil || strongSelf->_shuttingDown ||
            generation != strongSelf->_generation ||
            stream != strongSelf->_stream) {
          return;
        }
        if (startError != nil) {
          [strongSelf failCapture];
          return;
        }
        [strongSelf setCaptureState:PET_CAPTURE_READY];
      });
    }];
  }
}

- (void)stream:(SCStream *)stream
    didOutputSampleBuffer:(CMSampleBufferRef)sampleBuffer
                  ofType:(SCStreamOutputType)type {
  if (@available(macOS 12.3, *)) {
    if (type != SCStreamOutputTypeScreen ||
        !CMSampleBufferDataIsReady(sampleBuffer)) {
      return;
    }
    CVImageBufferRef imageBuffer = CMSampleBufferGetImageBuffer(sampleBuffer);
    if (imageBuffer == nullptr ||
        CFGetTypeID(imageBuffer) != CVPixelBufferGetTypeID()) {
      return;
    }
    IOSurfaceRef surface =
        CVPixelBufferGetIOSurface((CVPixelBufferRef)imageBuffer);
    if (surface == nullptr) {
      return;
    }

    CFRetain(surface);
    os_unfair_lock_lock(&_surfaceLock);
    const BOOL acceptFrame =
        _frameRetention.accepting() &&
        _activeStreamIdentity == (__bridge void *)stream;
    IOSurfaceRef previous = acceptFrame ? _latestSurface : nullptr;
    if (acceptFrame) {
      _latestSurface = surface;
    }
    os_unfair_lock_unlock(&_surfaceLock);
    if (!acceptFrame) {
      CFRelease(surface);
      return;
    }
    if (previous != nullptr) {
      CFRelease(previous);
    }
  }
}

- (void)stream:(SCStream *)stream
    didStopWithError:(NSError *)error API_AVAILABLE(macos(12.3)) {
  (void)error;
  dispatch_async(dispatch_get_main_queue(), ^{
    if (!self->_shuttingDown && stream == self->_stream) {
      [self failCapture];
    }
  });
}

- (void)stopStreamAndReleaseFrame {
  ++_generation;
  [self releaseLatestSurface];
  if (@available(macOS 12.3, *)) {
    SCStream *stream = _stream;
    _stream = nil;
    if (stream != nil) {
      [stream stopCaptureWithCompletionHandler:^(NSError *error) {
        (void)error;
        if (@available(macOS 12.3, *)) {
          NSError *removeError = nil;
          [stream removeStreamOutput:self
                                type:SCStreamOutputTypeScreen
                               error:&removeError];
        }
      }];
    }
  }
}

- (void)releaseLatestSurface {
  _frameRetention.stop();
  os_unfair_lock_lock(&_surfaceLock);
  IOSurfaceRef previous = _latestSurface;
  _latestSurface = nullptr;
  _activeStreamIdentity = nullptr;
  os_unfair_lock_unlock(&_surfaceLock);
  if (previous != nullptr) {
    CFRelease(previous);
  }
}

- (void)stop {
  [self stopStreamAndReleaseFrame];
}

- (void)shutdown {
  if (_shuttingDown) {
    return;
  }
  _shuttingDown = YES;
  [self stopStreamAndReleaseFrame];
  _callback = nullptr;
}

- (uint32_t)captureState {
  return _captureState;
}

- (IOSurfaceRef)copyLatestSurface {
  os_unfair_lock_lock(&_surfaceLock);
  IOSurfaceRef surface = _latestSurface;
  if (surface != nullptr) {
    CFRetain(surface);
  }
  os_unfair_lock_unlock(&_surfaceLock);
  return surface;
}

@end

extern "C" void *mac_capture_create(PetCallback callback) {
  id<BPCaptureControlling> service;
  if (@available(macOS 12.3, *)) {
    service = [[BPScreenCaptureService alloc] initWithCallback:callback];
  } else {
    service = [[BPUnavailableCaptureService alloc] initWithCallback:callback];
  }
  return (__bridge_retained void *)service;
}

extern "C" void mac_capture_destroy(void *handle) {
  if (handle == nullptr) {
    return;
  }
  id<BPCaptureControlling> service =
      (__bridge_transfer id<BPCaptureControlling>)handle;
  [service shutdown];
}

extern "C" void mac_capture_configure(
    void *handle, PetCaptureRegion region, bool real_mode, bool visible,
    bool request_permission, uint32_t fps) {
  if (handle == nullptr) {
    return;
  }
  id<BPCaptureControlling> service =
      (__bridge id<BPCaptureControlling>)handle;
  [service configureRegion:region
                  realMode:real_mode
                   visible:visible
         requestPermission:request_permission
                       fps:fps];
}

extern "C" void mac_capture_stop(void *handle) {
  if (handle == nullptr) {
    return;
  }
  [(__bridge id<BPCaptureControlling>)handle stop];
}

extern "C" uint32_t mac_capture_state(void *handle) {
  if (handle == nullptr) {
    return PET_CAPTURE_UNAVAILABLE;
  }
  return [(__bridge id<BPCaptureControlling>)handle captureState];
}

extern "C" IOSurfaceRef mac_capture_copy_latest_surface(void *handle) {
  if (handle == nullptr) {
    return nullptr;
  }
  return [(__bridge id<BPCaptureControlling>)handle copyLatestSurface];
}
