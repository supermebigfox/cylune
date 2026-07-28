#import "bridge.h"
#import "pet_lifecycle.h"

#import <CoreGraphics/CoreGraphics.h>
#import <CoreMedia/CoreMedia.h>
#import <CoreVideo/CoreVideo.h>
#import <IOSurface/IOSurface.h>
#import <QuartzCore/QuartzCore.h>
#import <ScreenCaptureKit/ScreenCaptureKit.h>
#import <dispatch/dispatch.h>
#import <os/lock.h>

#include <memory>
#include <vector>

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
- (uint32_t)shutdown;
- (uint32_t)captureState;
- (IOSurfaceRef)copyLatestSurfaceRegion:(PetCaptureRegion *)regionOut
    CF_RETURNS_RETAINED;
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

- (uint32_t)shutdown {
  _callback = nullptr;
  return PET_SHUTDOWN_COMPLETE;
}

- (uint32_t)captureState {
  return PET_CAPTURE_UNAVAILABLE;
}

- (IOSurfaceRef)copyLatestSurfaceRegion:(PetCaptureRegion *)regionOut {
  if (regionOut != nullptr) {
    *regionOut = {};
  }
  return nullptr;
}

@end

API_AVAILABLE(macos(12.3))
@interface BPScreenCaptureService
    : NSObject <BPCaptureControlling, SCStreamOutput, SCStreamDelegate>
- (instancetype)initWithCallback:(PetCallback)callback;
@end

static SCFrameStatus CaptureFrameStatus(
    CMSampleBufferRef sampleBuffer) API_AVAILABLE(macos(12.3)) {
  CFArrayRef attachments =
      CMSampleBufferGetSampleAttachmentsArray(sampleBuffer, false);
  if (attachments == nullptr || CFArrayGetCount(attachments) == 0) {
    return SCFrameStatusComplete;
  }
  NSDictionary *metadata =
      (__bridge NSDictionary *)CFArrayGetValueAtIndex(attachments, 0);
  NSNumber *status = metadata[SCStreamFrameInfoStatus];
  return status == nil ? SCFrameStatusComplete
                       : (SCFrameStatus)status.integerValue;
}

@implementation BPScreenCaptureService {
  PetCallback _callback;
  SCStream *_stream;
  dispatch_queue_t _frameQueue;
  os_unfair_lock _surfaceLock;
  IOSurfaceRef _latestSurface;
  PetCaptureRegion _latestSurfaceRegion;
  PetCaptureRegion _frameRegion;
  BOOL _hasFrameRegion;
  void *_activeStreamIdentity;
  PetFrameRetention _frameRetention;
  PetCaptureFrameFreshness _frameFreshness;
  NSUInteger _generation;
  uint32_t _captureState;
  PetCaptureRegion _activeRegion;
  PetCaptureRegion _desiredRegion;
  uint32_t _desiredFps;
  BOOL _hasActiveRegion;
  PetCaptureRestartGate _restartGate;
  PetCaptureReaimGate _reaimGate;
  PetPermissionLifecycle _permission;
  BOOL _shuttingDown;
  dispatch_queue_t _shutdownQueue;
  std::vector<std::shared_ptr<PetStopCompletion>> _pendingStops;
  PetFaultLatch _failure;
}

- (instancetype)initWithCallback:(PetCallback)callback {
  self = [super init];
  if (self) {
    _callback = callback;
    _frameQueue =
        dispatch_queue_create("com.local.bambuspools.capture.frames",
                              DISPATCH_QUEUE_SERIAL);
    _shutdownQueue =
        dispatch_queue_create("com.local.bambuspools.capture.shutdown",
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
  const BOOL shouldReport = _failure.report_once();
  [self stopStreamAndReleaseFrame];
  [self setCaptureState:PET_CAPTURE_FAILED];
  if (shouldReport && _callback != nullptr) {
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
    if (!PetShouldStartCapture(decision, visible)) {
      [self stopStreamAndReleaseFrame];
      return;
    }
    if (region.source_width <= 0.0 || region.source_height <= 0.0 ||
        region.pixel_width == 0 || region.pixel_height == 0) {
      [self failCapture];
      return;
    }
    _desiredRegion = region;
    _desiredFps = fps;
    if (_restartGate.in_flight()) {
      (void)_restartGate.request(region, fps);
      return;
    }

    BOOL supportsReaim = NO;
    if (@available(macOS 14.0, *)) {
      supportsReaim = YES;
    }
    const PetCaptureUpdateAction updateAction =
        _hasActiveRegion
            ? PetCaptureUpdateActionFor(
                  _activeRegion, region, _stream != nil,
                  supportsReaim != NO)
            : PetCaptureUpdateAction::kRestart;
    if (updateAction == PetCaptureUpdateAction::kNone) {
      return;
    }

    // Same-display drags keep the running stream and its newest valid frame.
    // Restarting here would briefly replace the lens with a transparent
    // texture, which reads as a large black ring when the mouse is released.
    if (updateAction == PetCaptureUpdateAction::kReaim) {
      if (@available(macOS 14.0, *)) {
        [self scheduleReaimRegion:region fps:fps];
        return;
      }
    }

    // BPPetHost calls this only for mode/visibility/display/region changes,
    // explicit user permission actions, wake, or display reconfiguration.
    // Those are the bounded retry points; ordinary FPS/pending updates never
    // reach this service.
    _failure.reset();
    [self requestRestartRegion:region fps:fps];
  } else {
    [self stopStreamAndReleaseFrame];
    [self setCaptureState:PET_CAPTURE_UNAVAILABLE];
  }
}

- (SCStreamConfiguration *)configurationForRegion:
                               (PetCaptureRegion)region
                                                fps:(uint32_t)fps
    API_AVAILABLE(macos(12.3)) {
  SCStreamConfiguration *configuration =
      [[SCStreamConfiguration alloc] init];
  configuration.sourceRect =
      CGRectMake(region.source_x, region.source_y, region.source_width,
                 region.source_height);
  configuration.width = region.pixel_width;
  configuration.height = region.pixel_height;
  configuration.pixelFormat = kCVPixelFormatType_32BGRA;
  configuration.queueDepth = PetSafeCapturePolicy().queue_depth;
  configuration.showsCursor = PetSafeCapturePolicy().shows_cursor;
  configuration.minimumFrameInterval =
      CMTimeMake(1, fps == 60 ? 60 : 30);
  if (@available(macOS 13.0, *)) {
    configuration.capturesAudio =
        PetSafeCapturePolicy().captures_audio;
    configuration.excludesCurrentProcessAudio = YES;
  }
  if (@available(macOS 15.0, *)) {
    configuration.captureMicrophone =
        PetSafeCapturePolicy().captures_microphone;
  }
  return configuration;
}

- (void)scheduleReaimRegion:(PetCaptureRegion)region
                        fps:(uint32_t)fps
    API_AVAILABLE(macos(14.0)) {
  _desiredRegion = region;
  _desiredFps = fps;
  const uint64_t token = _reaimGate.begin();
  if (token == 0) {
    return;
  }
  const NSUInteger generation = _generation;
  __weak BPScreenCaptureService *weakSelf = self;
  dispatch_after(
      dispatch_time(DISPATCH_TIME_NOW, 100 * NSEC_PER_MSEC),
      dispatch_get_main_queue(), ^{
        BPScreenCaptureService *strongSelf = weakSelf;
        if (strongSelf == nil || strongSelf->_shuttingDown ||
            generation != strongSelf->_generation ||
            strongSelf->_stream == nil ||
            !strongSelf->_reaimGate.owns(token)) {
          if (strongSelf != nil &&
              strongSelf->_reaimGate.owns(token)) {
            (void)strongSelf->_reaimGate.complete(token);
          }
          return;
        }
        SCStream *stream = strongSelf->_stream;
        const PetCaptureRegion appliedRegion =
            strongSelf->_desiredRegion;
        const uint32_t appliedFps = strongSelf->_desiredFps;
        SCStreamConfiguration *configuration =
            [strongSelf configurationForRegion:appliedRegion
                                           fps:appliedFps];
        [stream
            updateConfiguration:configuration
              completionHandler:^(NSError *error) {
                dispatch_async(dispatch_get_main_queue(), ^{
                  BPScreenCaptureService *completedSelf = weakSelf;
                  if (completedSelf == nil ||
                      completedSelf->_shuttingDown ||
                      generation != completedSelf->_generation ||
                      stream != completedSelf->_stream ||
                      !completedSelf->_reaimGate.owns(token)) {
                    return;
                  }
                  if (error != nil) {
                    const PetCaptureRegion latest =
                        completedSelf->_desiredRegion;
                    const uint32_t latestFps =
                        completedSelf->_desiredFps;
                    (void)completedSelf->_reaimGate.complete(token);
                    [completedSelf requestRestartRegion:latest
                                                    fps:latestFps];
                    return;
                  }
                  dispatch_async(completedSelf->_frameQueue, ^{
                    BPScreenCaptureService *frameSelf = weakSelf;
                    if (frameSelf == nil) {
                      return;
                    }
                    frameSelf->_frameRegion = appliedRegion;
                    frameSelf->_hasFrameRegion = YES;
                    dispatch_async(dispatch_get_main_queue(), ^{
                      BPScreenCaptureService *finishedSelf = weakSelf;
                      if (finishedSelf == nil ||
                          finishedSelf->_shuttingDown ||
                          generation != finishedSelf->_generation ||
                          stream != finishedSelf->_stream ||
                          !finishedSelf->_reaimGate.complete(token)) {
                        return;
                      }
                      finishedSelf->_activeRegion = appliedRegion;
                      finishedSelf->_hasActiveRegion = YES;
                      if (!PetCaptureRegionsEqual(
                              appliedRegion,
                              finishedSelf->_desiredRegion)) {
                        [finishedSelf
                            scheduleReaimRegion:
                                finishedSelf->_desiredRegion
                                              fps:
                                finishedSelf->_desiredFps];
                      }
                    });
                  });
                });
              }];
      });
}

- (void)requestRestartRegion:(PetCaptureRegion)region
                          fps:(uint32_t)fps {
  if (@available(macOS 12.3, *)) {
    [self stopStreamAndReleaseFrame];
    if (!_restartGate.request(region, fps)) {
      return;
    }
    _desiredRegion = region;
    _desiredFps = fps;
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
          strongSelf->_restartGate.complete();
          [strongSelf failCapture];
          return;
        }
        const PetCaptureRegion latestRegion =
            strongSelf->_restartGate.desired_region();
        const uint32_t latestFps =
            strongSelf->_restartGate.desired_fps();
        [strongSelf startWithContent:content region:latestRegion fps:latestFps
                         generation:generation];
      });
    }];
  }
}

- (void)reconcileDesiredCapture API_AVAILABLE(macos(12.3)) {
  if (_stream == nil || !_hasActiveRegion ||
      PetCaptureRegionsEqual(_activeRegion, _desiredRegion)) {
    return;
  }
  if (@available(macOS 14.0, *)) {
    if (_activeRegion.display_id == _desiredRegion.display_id) {
      [self scheduleReaimRegion:_desiredRegion fps:_desiredFps];
      return;
    }
  }
  [self requestRestartRegion:_desiredRegion fps:_desiredFps];
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
      _restartGate.complete();
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
        [self configurationForRegion:region fps:fps];

    SCStream *stream = [[SCStream alloc] initWithFilter:filter
                                          configuration:configuration
                                               delegate:self];
    NSError *outputError = nil;
    if (![stream addStreamOutput:self
                            type:SCStreamOutputTypeScreen
              sampleHandlerQueue:_frameQueue
                           error:&outputError] ||
        outputError != nil) {
      _restartGate.complete();
      [self failCapture];
      return;
    }

    dispatch_sync(_frameQueue, ^{
      self->_frameRegion = region;
      self->_hasFrameRegion = YES;
    });
    _stream = stream;
    _activeRegion = region;
    _hasActiveRegion = YES;
    os_unfair_lock_lock(&_surfaceLock);
    _activeStreamIdentity = (__bridge void *)stream;
    _frameRetention.start();
    _frameFreshness.invalidate();
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
          strongSelf->_restartGate.complete();
          [strongSelf failCapture];
          return;
        }
        strongSelf->_restartGate.complete();
        [strongSelf setCaptureState:PET_CAPTURE_READY];
        [strongSelf reconcileDesiredCapture];
      });
    }];
  }
}

- (void)stream:(SCStream *)stream
    didOutputSampleBuffer:(CMSampleBufferRef)sampleBuffer
                  ofType:(SCStreamOutputType)type {
  if (@available(macOS 12.3, *)) {
    if (type != SCStreamOutputTypeScreen) {
      return;
    }
    const SCFrameStatus status = CaptureFrameStatus(sampleBuffer);
    const double observedAt = CACurrentMediaTime();
    if (status == SCFrameStatusIdle) {
      os_unfair_lock_lock(&_surfaceLock);
      if (_frameRetention.accepting() &&
          _activeStreamIdentity == (__bridge void *)stream &&
          _latestSurface != nullptr) {
        _frameFreshness.idle_at(observedAt);
      }
      os_unfair_lock_unlock(&_surfaceLock);
      return;
    }
    if (status != SCFrameStatusComplete &&
        status != SCFrameStatusStarted) {
      [self clearLatestSurfacePreservingStream];
      return;
    }
    if (!CMSampleBufferDataIsReady(sampleBuffer)) {
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
    const BOOL dimensionsMatch =
        _hasFrameRegion &&
        IOSurfaceGetWidth(surface) == _frameRegion.pixel_width &&
        IOSurfaceGetHeight(surface) == _frameRegion.pixel_height;

    CFRetain(surface);
    os_unfair_lock_lock(&_surfaceLock);
    const BOOL acceptFrame =
        _frameRetention.accepting() &&
        _activeStreamIdentity == (__bridge void *)stream &&
        dimensionsMatch;
    IOSurfaceRef previous = acceptFrame ? _latestSurface : nullptr;
    if (acceptFrame) {
      _latestSurface = surface;
      _latestSurfaceRegion = _frameRegion;
      _frameFreshness.complete_at(observedAt);
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

- (void)clearLatestSurfacePreservingStream {
  os_unfair_lock_lock(&_surfaceLock);
  IOSurfaceRef previous = _latestSurface;
  _latestSurface = nullptr;
  _latestSurfaceRegion = {};
  _frameFreshness.invalidate();
  os_unfair_lock_unlock(&_surfaceLock);
  if (previous != nullptr) {
    CFRelease(previous);
  }
}

- (void)stream:(SCStream *)stream
    didStopWithError:(NSError *)error API_AVAILABLE(macos(12.3)) {
  (void)error;
  __weak BPScreenCaptureService *weakSelf = self;
  dispatch_async(dispatch_get_main_queue(), ^{
    BPScreenCaptureService *strongSelf = weakSelf;
    if (strongSelf != nil && !strongSelf->_shuttingDown &&
        stream == strongSelf->_stream) {
      [strongSelf failCapture];
    }
  });
}

- (void)stopStreamAndReleaseFrame {
  ++_generation;
  _reaimGate.cancel();
  _restartGate.cancel();
  _hasActiveRegion = NO;
  dispatch_async(_frameQueue, ^{
    self->_hasFrameRegion = NO;
  });
  [self releaseLatestSurface];
  if (@available(macOS 12.3, *)) {
    SCStream *stream = _stream;
    _stream = nil;
    if (stream != nil) {
      auto completion = std::make_shared<PetStopCompletion>();
      _pendingStops.push_back(completion);
      [stream stopCaptureWithCompletionHandler:^(NSError *error) {
        if (@available(macOS 12.3, *)) {
          NSError *removeError = nil;
          [stream removeStreamOutput:self
                                type:SCStreamOutputTypeScreen
                               error:&removeError];
        }
        completion->complete(error == nil);
      }];
    }
  }
}

- (void)releaseLatestSurface {
  _frameRetention.stop();
  os_unfair_lock_lock(&_surfaceLock);
  IOSurfaceRef previous = _latestSurface;
  _latestSurface = nullptr;
  _latestSurfaceRegion = {};
  _frameFreshness.invalidate();
  _activeStreamIdentity = nullptr;
  os_unfair_lock_unlock(&_surfaceLock);
  if (previous != nullptr) {
    CFRelease(previous);
  }
}

- (void)stop {
  [self stopStreamAndReleaseFrame];
}

- (uint32_t)shutdown {
  if (_shuttingDown) {
    return PET_SHUTDOWN_COMPLETE;
  }
  _shuttingDown = YES;
  _callback = nullptr;
  ++_generation;
  _reaimGate.cancel();
  _restartGate.cancel();
  _hasActiveRegion = NO;

  _frameRetention.stop();
  os_unfair_lock_lock(&_surfaceLock);
  _activeStreamIdentity = nullptr;
  _frameFreshness.invalidate();
  os_unfair_lock_unlock(&_surfaceLock);

  if (@available(macOS 12.3, *)) {
    SCStream *stream = _stream;
    _stream = nil;
    if (stream != nil) {
      auto completion = std::make_shared<PetStopCompletion>();
      _pendingStops.push_back(completion);
      dispatch_async(_shutdownQueue, ^{
        [stream stopCaptureWithCompletionHandler:^(NSError *error) {
          if (@available(macOS 12.3, *)) {
            NSError *removeError = nil;
            [stream removeStreamOutput:self
                                  type:SCStreamOutputTypeScreen
                                 error:&removeError];
          }
          completion->complete(error == nil);
        }];
      });
    }
  }

  const std::vector<std::shared_ptr<PetStopCompletion>> pending =
      _pendingStops;
  __block PetShutdownState result = PetShutdownState::kComplete;
  // ScreenCaptureKit does not promise a completion queue. Start the final
  // stop and perform the bounded wait on this service-owned queue; the
  // completion touches only the stream/output and never calls AppKit.
  dispatch_sync(_shutdownQueue, ^{
    const auto deadline =
        std::chrono::steady_clock::now() + PetCaptureShutdownTimeout();
    for (const auto &completion : pending) {
      const auto now = std::chrono::steady_clock::now();
      const auto remaining =
          now < deadline
              ? std::chrono::duration_cast<std::chrono::milliseconds>(
                    deadline - now)
              : std::chrono::milliseconds::zero();
      const PetShutdownState state = completion->wait_for(remaining);
      if (state == PetShutdownState::kStopTimedOut) {
        result = state;
      } else if (state == PetShutdownState::kStopFailed &&
                 result == PetShutdownState::kComplete) {
        result = state;
      }
    }
  });
  [self releaseLatestSurface];
  return static_cast<uint32_t>(result);
}

- (uint32_t)captureState {
  return _captureState;
}

- (IOSurfaceRef)copyLatestSurfaceRegion:(PetCaptureRegion *)regionOut {
  if (regionOut != nullptr) {
    *regionOut = {};
  }
  os_unfair_lock_lock(&_surfaceLock);
  IOSurfaceRef surface = _latestSurface;
  IOSurfaceRef expired = nullptr;
  if (surface != nullptr &&
      !_frameFreshness.reusable_at(CACurrentMediaTime())) {
    expired = surface;
    surface = nullptr;
    _latestSurface = nullptr;
    _latestSurfaceRegion = {};
    _frameFreshness.invalidate();
  } else if (surface != nullptr) {
    CFRetain(surface);
    if (regionOut != nullptr) {
      *regionOut = _latestSurfaceRegion;
    }
  }
  os_unfair_lock_unlock(&_surfaceLock);
  if (expired != nullptr) {
    CFRelease(expired);
  }
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

extern "C" uint32_t mac_capture_destroy(void *handle) {
  if (handle == nullptr) {
    return PET_SHUTDOWN_COMPLETE;
  }
  id<BPCaptureControlling> service =
      (__bridge_transfer id<BPCaptureControlling>)handle;
  return [service shutdown];
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

extern "C" IOSurfaceRef mac_capture_copy_latest_surface(
    void *handle, PetCaptureRegion *region_out) {
  if (region_out != nullptr) {
    *region_out = {};
  }
  if (handle == nullptr) {
    return nullptr;
  }
  return [(__bridge id<BPCaptureControlling>)handle
      copyLatestSurfaceRegion:region_out];
}
