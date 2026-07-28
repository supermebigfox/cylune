#import "bridge.h"
#import "pet_lifecycle.h"
#import "pet_render_state.h"
#import "pet_visual_state.h"

#import <AppKit/AppKit.h>
#import <CoreVideo/CoreVideo.h>
#import <Metal/Metal.h>
#import <QuartzCore/QuartzCore.h>
#import <dispatch/dispatch.h>

#include <math.h>
#include <stddef.h>
#include <sys/stat.h>
#include <atomic>
#include <vector>

// CVDisplayLink remains the frame clock on macOS 10.15–14. The replacement
// NSDisplayLink APIs start at macOS 14, so suppress only this intentional SDK
// deprecation while retaining the project's 10.15 deployment target.
#pragma clang diagnostic push
#pragma clang diagnostic ignored "-Wdeprecated-declarations"

static const uint32_t kPetAbiVersion = 1;
static const uint32_t kPetCallbackClicked = 1;
static const uint32_t kPetCallbackMoved = 2;
static const uint32_t kPetCallbackDropEntered = 3;
static const uint32_t kPetCallbackDropExited = 4;
static const uint32_t kPetCallbackFileDropped = 5;
static const uint32_t kPetCallbackDisplayChanged = 6;
static const uint32_t kPetCallbackCaptureFailed = 8;
static const uint32_t kPetCallbackSleep = 9;
static const uint32_t kPetCallbackWake = 10;
static const CGFloat kPetMinimumSize = 120.0;
static const CGFloat kPetMaximumSize = 900.0;
static const CGFloat kPetDragThreshold = 4.0;

static NSWindowLevel PetVisualWindowLevel(
    PetWindowPresentation presentation) {
  return presentation.layer == PetWindowLayer::kFloating
             ? NSFloatingWindowLevel
             : CGWindowLevelForKey(kCGDesktopIconWindowLevelKey) + 1;
}

static NSWindowCollectionBehavior PetWindowBehavior(
    PetWindowPresentation presentation) {
  NSWindowCollectionBehavior behavior =
      NSWindowCollectionBehaviorCanJoinAllSpaces |
      NSWindowCollectionBehaviorStationary |
      NSWindowCollectionBehaviorIgnoresCycle;
  if (presentation.full_screen_auxiliary) {
    behavior |= NSWindowCollectionBehaviorFullScreenAuxiliary;
  }
  return behavior;
}

typedef struct {
  BOOL valid;
  uint32_t fileKind;
} BPDropCandidateKind;

static_assert(sizeof(PetConfig) == 64, "PetConfig ABI size changed");
static_assert(alignof(PetConfig) == 8, "PetConfig ABI alignment changed");
static_assert(offsetof(PetConfig, abi_version) == 0,
              "PetConfig abi_version offset changed");
static_assert(offsetof(PetConfig, mode) == 4,
              "PetConfig mode offset changed");
static_assert(offsetof(PetConfig, effective_mode) == 8,
              "PetConfig effective_mode offset changed");
static_assert(offsetof(PetConfig, has_position) == 12,
              "PetConfig has_position offset changed");
static_assert(offsetof(PetConfig, size) == 16,
              "PetConfig size offset changed");
static_assert(offsetof(PetConfig, x) == 24,
              "PetConfig x offset changed");
static_assert(offsetof(PetConfig, y) == 32,
              "PetConfig y offset changed");
static_assert(offsetof(PetConfig, display_id) == 40,
              "PetConfig display_id offset changed");
static_assert(offsetof(PetConfig, fps) == 48,
              "PetConfig fps offset changed");
static_assert(offsetof(PetConfig, visible) == 52,
              "PetConfig visible offset changed");
static_assert(offsetof(PetConfig, pending_count) == 56,
              "PetConfig pending_count offset changed");
static_assert(offsetof(PetConfig, reduce_motion) == 60,
              "PetConfig reduce_motion offset changed");
static_assert(offsetof(PetConfig, request_permission) == 61,
              "PetConfig request_permission offset changed");
static_assert(offsetof(PetConfig, visual_style) == 62,
              "PetConfig visual_style offset changed");

@class BPPetHost;
@class BPDisplayPane;

@interface BPPetPanel : NSPanel
@end

@interface BPPetView : NSView
@property(nonatomic, weak) BPPetHost *petHost;
- (instancetype)initWithFrame:(NSRect)frame metalSource:(NSString *)metalSource;
- (BOOL)metalAvailable;
- (void)setMode:(uint32_t)mode;
- (void)setVisualStyle:(uint8_t)visualStyle;
- (void)setFps:(uint32_t)fps;
- (void)setReduceMotion:(BOOL)reduceMotion;
- (void)setAnimating:(BOOL)animating;
- (void)setHovering:(BOOL)hovering;
- (void)completeDrop;
- (BOOL)beginImportWait:(uint64_t)generation
                 origin:(NSPoint)origin
               fileKind:(uint32_t)fileKind;
- (BOOL)finishImport:(uint64_t)generation result:(uint32_t)result;
- (void)cancelImport;
- (void)setPendingCount:(uint32_t)pendingCount;
- (void)signal:(uint32_t)signal;
- (void)pulse;
- (void)displayLinkTick:(const CVTimeStamp *)outputTime;
- (void)renderFrame;
- (void)updateDrawableSize;
- (void)setCenterUVX:(CGFloat)x
                   y:(CGFloat)y
          effectSize:(CGFloat)effectSize
       displayHeight:(CGFloat)displayHeight;
@end

@interface BPCoreHitTargetView : NSView
@property(nonatomic, weak) BPPetHost *petHost;
@property(nonatomic, assign) PetCallback callback;
@end

@interface BPPetHost : NSObject
@property(nonatomic, strong) BPPetPanel *panel;
@property(nonatomic, strong) BPPetPanel *coreHitTargetPanel;
@property(nonatomic, strong) BPPetView *petView;
@property(nonatomic, strong) BPCoreHitTargetView *coreHitTargetView;
@property(nonatomic, assign) NSPoint centerScreenPoint;
@property(nonatomic, assign) CGFloat effectSize;
@property(nonatomic, assign) BOOL gestureActive;
@property(nonatomic, assign) NSPoint gestureMouseOrigin;
@property(nonatomic, assign) NSPoint gesturePanelOrigin;
@property(nonatomic, assign) BOOL gestureMoved;
@property(nonatomic, assign) uint64_t displayID;
@property(nonatomic, assign) BOOL observingScreenChanges;
@property(nonatomic, assign) BOOL observingWorkspace;
- (instancetype)initWithCallback:(PetCallback)callback
                      metalSource:(NSString *)metalSource;
- (void)applyConfig:(PetConfig)config;
- (void)show;
- (void)showWithPermissionRequest:(BOOL)requestPermission;
- (void)hide;
- (void)reset;
- (void)signal:(uint32_t)signal;
- (void)beginGestureAt:(NSPoint)screenPoint;
- (void)continueGestureAt:(NSPoint)screenPoint;
- (void)endGesture;
- (void)dragEntered;
- (void)dragExited;
- (void)dropCompleted;
- (void)syncCoreHitTargetFrame;
- (void)rebuildDisplayPanes;
- (void)updatePaneGeometry;
- (NSScreen *)screenForPanel;
- (void)updateDisplaySelectionAndEmit:(BOOL)emit;
- (void)screenParametersChanged:(NSNotification *)notification;
- (void)workspaceWillSleep:(NSNotification *)notification;
- (void)workspaceDidWake:(NSNotification *)notification;
- (void)refreshCaptureWithPermissionRequest:(BOOL)requestPermission;
- (void)rendererBecameUnavailable;
- (IOSurfaceRef)copyLatestSurfaceForView:(BPPetView *)view
                                  region:(PetCaptureRegion *)regionOut
    CF_RETURNS_RETAINED;
- (uint32_t)captureState;
- (uint32_t)rendererState;
- (uint32_t)shutdown;
@end

@interface BPDisplayPane : NSObject
@property(nonatomic, strong) BPPetPanel *panel;
@property(nonatomic, strong) BPPetView *petView;
@property(nonatomic, assign) void *captureHandle;
@property(nonatomic, assign) uint64_t displayID;
@property(nonatomic, assign) NSRect screenFrame;
@property(nonatomic, assign) PetCaptureRegion captureRegion;
@end

@interface BPPetBridge : NSObject
@property(nonatomic, assign) PetCallback callback;
@property(nonatomic, strong) BPPetHost *host;
@property(nonatomic, copy) NSString *metalSource;
@property(nonatomic, assign) BOOL destroyed;
@property(nonatomic, assign) uint32_t shutdownState;
- (instancetype)initWithCallback:(PetCallback)callback
                      metalSource:(NSString *)metalSource;
- (void)ensureHost;
- (uint64_t)issueApplyGeneration;
- (BOOL)acceptApplyGeneration:(uint64_t)generation;
- (uint32_t)shutdown;
@end

@implementation BPPetPanel

- (BOOL)canBecomeKeyWindow {
  return NO;
}

- (BOOL)canBecomeMainWindow {
  return NO;
}

@end

@implementation BPDisplayPane
@end

static CVReturn PetDisplayLinkCallback(CVDisplayLinkRef displayLink,
                                       const CVTimeStamp *now,
                                       const CVTimeStamp *outputTime,
                                       CVOptionFlags flagsIn,
                                       CVOptionFlags *flagsOut,
                                       void *displayLinkContext) {
  (void)displayLink;
  (void)now;
  (void)flagsIn;
  (void)flagsOut;
  @autoreleasepool {
    BPPetView *view = (__bridge BPPetView *)displayLinkContext;
    [view displayLinkTick:outputTime];
  }
  return kCVReturnSuccess;
}

static void *ProductionRendererCreate(void *context, const char *source,
                                      void *layer) {
  (void)context;
  return mac_renderer_create(source, layer);
}

static uint32_t ProductionRendererDraw(void *context, void *handle,
                                       IOSurfaceRef surface,
                                       PetRenderUniforms uniforms) {
  (void)context;
  return mac_renderer_draw(handle, surface, uniforms);
}

static void ProductionRendererDestroy(void *context, void *handle) {
  (void)context;
  mac_renderer_destroy(handle);
}

static PetRendererBackend ProductionRendererBackend() {
  return {nullptr, ProductionRendererCreate, ProductionRendererDraw,
          ProductionRendererDestroy};
}

@implementation BPPetView {
  __weak BPPetHost *_petHost;
  CALayer *_diskLayer;
  CAGradientLayer *_ringLayer;
  CAShapeLayer *_ringMask;
  CALayer *_pendingDotsLayer;
  NSMutableArray<CALayer *> *_pendingDotLayers;
  CAShapeLayer *_signalLayer;
  CAShapeLayer *_dropCardLayer;
  PetVisualState _visualState;
  BOOL _reduceMotion;
  BOOL _animating;
  BOOL _metalAvailable;
  NSString *_metalSource;
  PetRendererDriver _rendererDriver;
  CVDisplayLinkRef _displayLink;
  PetFrameDispatchGate _frameGate;
  CFTimeInterval _renderEpoch;
  CFTimeInterval _lastRenderedAt;
  uint32_t _mode;
  uint32_t _visualStyle;
  uint32_t _fps;
  CGFloat _centerUVX;
  CGFloat _centerUVY;
  CGFloat _effectSize;
  CGFloat _displayHeight;
  PetRenderAnimationState _renderAnimation;
  PetDropState _dropState;
  PetImpactState _impactState;
}

- (instancetype)initWithFrame:(NSRect)frame {
  return [self initWithFrame:frame metalSource:@""];
}

- (instancetype)initWithFrame:(NSRect)frame metalSource:(NSString *)metalSource {
  self = [super initWithFrame:frame];
  if (self) {
    _metalSource = [metalSource copy];
    _fps = 0;
    _mode = 1;
    _visualStyle = 0;
    _centerUVX = 0.5;
    _centerUVY = 0.5;
    _effectSize = 220.0;
    _displayHeight = MAX(NSHeight(frame), 1.0);
    _renderEpoch = CACurrentMediaTime();
    self.wantsLayer = YES;
    self.layer.backgroundColor = NSColor.clearColor.CGColor;

    _diskLayer = [CALayer layer];
    _diskLayer.backgroundColor = NSColor.blackColor.CGColor;
    [self.layer addSublayer:_diskLayer];

    _ringLayer = [CAGradientLayer layer];
    _ringLayer.colors = @[
      (__bridge id)[NSColor colorWithSRGBRed:0.20 green:0.85 blue:1.0 alpha:1.0].CGColor,
      (__bridge id)[NSColor colorWithSRGBRed:0.72 green:0.28 blue:1.0 alpha:1.0].CGColor,
      (__bridge id)[NSColor colorWithSRGBRed:1.0 green:0.32 blue:0.56 alpha:1.0].CGColor
    ];
    _ringLayer.startPoint = CGPointMake(0.0, 0.5);
    _ringLayer.endPoint = CGPointMake(1.0, 0.5);
    _ringMask = [CAShapeLayer layer];
    _ringMask.fillColor = NSColor.clearColor.CGColor;
    _ringMask.strokeColor = NSColor.whiteColor.CGColor;
    _ringMask.lineCap = kCALineCapRound;
    _ringLayer.mask = _ringMask;
    [self.layer addSublayer:_ringLayer];

    _pendingDotsLayer = [CALayer layer];
    _pendingDotLayers = [NSMutableArray array];
    [self.layer addSublayer:_pendingDotsLayer];

    _signalLayer = [CAShapeLayer layer];
    _signalLayer.fillColor = NSColor.clearColor.CGColor;
    _signalLayer.strokeColor = NSColor.clearColor.CGColor;
    _signalLayer.lineCap = kCALineCapRound;
    _signalLayer.opacity = 0.0;
    [self.layer addSublayer:_signalLayer];

    _dropCardLayer = [CAShapeLayer layer];
    _dropCardLayer.fillColor =
        [NSColor colorWithSRGBRed:0.22 green:0.68 blue:1.0 alpha:1.0]
            .CGColor;
    _dropCardLayer.strokeColor =
        [NSColor colorWithSRGBRed:0.78 green:0.92 blue:1.0 alpha:1.0]
            .CGColor;
    _dropCardLayer.lineWidth = 1.5;
    _dropCardLayer.opacity = 0.0;
    [self.layer addSublayer:_dropCardLayer];

    if (CVDisplayLinkCreateWithActiveCGDisplays(&_displayLink) ==
        kCVReturnSuccess) {
      CVDisplayLinkSetOutputCallback(_displayLink, PetDisplayLinkCallback,
                                     (__bridge void *)self);
    }
  }
  return self;
}

- (void)dealloc {
  _dropState.cancel();
  _impactState.clear();
  [_dropCardLayer removeAllAnimations];
  if (_displayLink != nullptr) {
    CVDisplayLinkStop(_displayLink);
    CVDisplayLinkRelease(_displayLink);
    _displayLink = nullptr;
  }
  _rendererDriver.shutdown();
}

- (CALayer *)makeBackingLayer {
  id<MTLDevice> device = MTLCreateSystemDefaultDevice();
  if (device != nil) {
    CAMetalLayer *metalLayer = [CAMetalLayer layer];
    metalLayer.device = device;
    metalLayer.pixelFormat = MTLPixelFormatBGRA8Unorm;
    metalLayer.framebufferOnly = YES;
    metalLayer.opaque = NO;
    _metalAvailable = _rendererDriver.initialize(
        ProductionRendererBackend(), _metalSource.UTF8String,
        (__bridge void *)metalLayer);
    return metalLayer;
  }
  _metalAvailable = _rendererDriver.initialize(
      ProductionRendererBackend(), _metalSource.UTF8String, nullptr);
  return [CALayer layer];
}

- (void)setPetHost:(BPPetHost *)petHost {
  _petHost = petHost;
  if (petHost != nil &&
      _rendererDriver.bind_host() ==
          PetRendererStep::kBecameUnavailable) {
    [petHost rendererBecameUnavailable];
  }
}

- (BPPetHost *)petHost {
  return _petHost;
}

- (BOOL)metalAvailable {
  return _metalAvailable;
}

- (BOOL)isFlipped {
  return YES;
}

- (void)layout {
  [super layout];
  const CGRect bounds = self.bounds;
  [self updateDrawableSize];
  const CGFloat panelSide = _effectSize;
  const PetEffectGeometry geometry =
      PetEffectGeometryForSize(panelSide);
  const CGFloat ringWidth = MAX(3.0, panelSide * 0.035);
  const CGRect effectFrame =
      CGRectMake(CGRectGetWidth(bounds) * _centerUVX -
                     geometry.panel_side / 2.0,
                 CGRectGetHeight(bounds) * _centerUVY -
                     geometry.panel_side / 2.0,
                 geometry.panel_side,
                 geometry.panel_side);
  const CGRect shadowFrame =
      CGRectMake(CGRectGetWidth(bounds) * _centerUVX -
                     geometry.shadow_radius,
                 CGRectGetHeight(bounds) * _centerUVY -
                     geometry.shadow_radius,
                 geometry.shadow_radius * 2.0,
                 geometry.shadow_radius * 2.0);

  [CATransaction begin];
  [CATransaction setDisableActions:YES];
  _diskLayer.hidden = _metalAvailable;
  _ringLayer.hidden = _metalAvailable;
  _pendingDotsLayer.hidden = _metalAvailable;
  _signalLayer.hidden = _metalAvailable;
  _dropCardLayer.hidden = _metalAvailable;
  _diskLayer.frame = shadowFrame;
  _diskLayer.cornerRadius = geometry.shadow_radius;
  _ringLayer.frame = bounds;
  _ringMask.frame = bounds;
  _ringMask.lineWidth = ringWidth;
  // The full-size lens/accretion ring is decorative and click-through. The
  // smaller black event horizon is the visible drop/drag target.
  CGPathRef ringPath =
      CGPathCreateWithEllipseInRect(
          CGRectInset(effectFrame, ringWidth / 2.0, ringWidth / 2.0), nullptr);
  _ringMask.path = ringPath;
  CGPathRelease(ringPath);

  _pendingDotsLayer.frame = bounds;
  const CGFloat dotDiameter =
      MIN(8.0, MAX(4.0, panelSide * 0.032));
  const uint32_t pendingCount = _visualState.pending_dot_count();
  for (uint32_t index = 0; index < pendingCount; ++index) {
    const PetPendingDotPlacement placement =
        PetPendingDotPlacementForIndex(index, pendingCount);
    const CGFloat orbitRadius =
        panelSide * 0.5 * placement.normalized_radius;
    const CGFloat centerX =
        CGRectGetWidth(bounds) * _centerUVX +
        cos(placement.angle_radians) * orbitRadius;
    const CGFloat centerY =
        CGRectGetHeight(bounds) * _centerUVY +
        sin(placement.angle_radians) * orbitRadius;
    CALayer *dot = _pendingDotLayers[index];
    dot.frame =
        CGRectMake(centerX - dotDiameter / 2.0,
                   centerY - dotDiameter / 2.0, dotDiameter, dotDiameter);
    dot.cornerRadius = dotDiameter / 2.0;
  }

  _signalLayer.frame = bounds;
  _signalLayer.lineWidth = MAX(4.0, ringWidth * 1.25);
  CGPathRef signalPath =
      CGPathCreateWithEllipseInRect(
          CGRectInset(effectFrame, ringWidth * 1.5, ringWidth * 1.5),
          nullptr);
  _signalLayer.path = signalPath;
  CGPathRelease(signalPath);

  const CGSize cardSize =
      CGSizeMake(MAX(34.0, panelSide * 0.29),
                 MAX(24.0, panelSide * 0.20));
  _dropCardLayer.bounds =
      CGRectMake(0.0, 0.0, cardSize.width, cardSize.height);
  CGPathRef cardPath = CGPathCreateWithRoundedRect(
      _dropCardLayer.bounds, cardSize.height * 0.18,
      cardSize.height * 0.18, nullptr);
  _dropCardLayer.path = cardPath;
  CGPathRelease(cardPath);
  [CATransaction commit];
}

- (void)viewDidChangeBackingProperties {
  [super viewDidChangeBackingProperties];
  [self updateDrawableSize];
}

- (void)updateDrawableSize {
  if (!_metalAvailable || ![self.layer isKindOfClass:CAMetalLayer.class]) {
    return;
  }
  CAMetalLayer *metalLayer = (CAMetalLayer *)self.layer;
  CGFloat scale = self.window.backingScaleFactor;
  if (scale <= 0.0) {
    NSScreen *primary = NSScreen.screens.firstObject;
    scale = primary == nil ? 1.0 : primary.backingScaleFactor;
  }
  const CGRect bounds = self.bounds;
  const PetDrawableMetrics metrics = PetDrawableMetricsForLogicalSize(
      CGRectGetWidth(bounds), CGRectGetHeight(bounds), scale);
  metalLayer.contentsScale = metrics.contents_scale;
  metalLayer.drawableSize =
      CGSizeMake(metrics.pixel_width, metrics.pixel_height);
}

- (void)setCenterUVX:(CGFloat)x
                   y:(CGFloat)y
          effectSize:(CGFloat)effectSize
       displayHeight:(CGFloat)displayHeight {
  _centerUVX = x;
  _centerUVY = y;
  _effectSize = effectSize;
  _displayHeight = MAX(displayHeight, 1.0);
  _lastRenderedAt = 0.0;
  [self setNeedsLayout:YES];
}

- (void)setMode:(uint32_t)mode {
  _mode = mode == 0 ? 0 : 1;
}

- (void)setVisualStyle:(uint8_t)visualStyle {
  _visualStyle = visualStyle == 1 ? 1 : 0;
  _lastRenderedAt = 0.0;
}

- (void)setFps:(uint32_t)fps {
  _fps = fps == 30 || fps == 60 ? fps : 0;
  _lastRenderedAt = 0.0;
}

- (void)setHovering:(BOOL)hovering {
  _renderAnimation.set_hover(hovering, CACurrentMediaTime());
  if (!_metalAvailable && hovering) {
    [self pulse];
  }
}

- (void)completeDrop {
  _renderAnimation.complete_drop(CACurrentMediaTime());
}

- (NSPoint)pointForDropOrigin:(PetDropOrigin)origin {
  const NSRect bounds = self.bounds;
  return NSMakePoint(
      NSMinX(bounds) + origin.x * NSWidth(bounds),
      NSMinY(bounds) + origin.y * NSHeight(bounds));
}

- (void)showFallbackCardAt:(NSPoint)origin
                  fileKind:(uint32_t)fileKind {
  [_dropCardLayer removeAllAnimations];
  [CATransaction begin];
  [CATransaction setDisableActions:YES];
  _dropCardLayer.fillColor =
      (fileKind == PET_FILE_GCODE
           ? [NSColor colorWithSRGBRed:0.20 green:0.86 blue:0.53 alpha:1.0]
           : [NSColor colorWithSRGBRed:0.22 green:0.68 blue:1.0 alpha:1.0])
          .CGColor;
  _dropCardLayer.strokeColor =
      [NSColor colorWithSRGBRed:0.84 green:0.95 blue:1.0 alpha:1.0]
          .CGColor;
  _dropCardLayer.position = origin;
  _dropCardLayer.opacity = 1.0;
  [CATransaction commit];
}

- (BOOL)beginImportWait:(uint64_t)generation
                 origin:(NSPoint)origin
               fileKind:(uint32_t)fileKind {
  const NSRect bounds = self.bounds;
  if (NSWidth(bounds) <= 0.0 || NSHeight(bounds) <= 0.0) {
    return NO;
  }
  const PetDropOrigin normalized = {
      (float)std::clamp((origin.x - NSMinX(bounds)) / NSWidth(bounds),
                        0.0, 1.0),
      (float)std::clamp((origin.y - NSMinY(bounds)) / NSHeight(bounds),
                        0.0, 1.0),
  };
  if (!_dropState.begin_wait(generation, normalized, fileKind,
                             CACurrentMediaTime())) {
    return NO;
  }
  _impactState.clear();
  _lastRenderedAt = 0.0;
  if (_metalAvailable) {
    if (_animating) {
      [self renderFrame];
    }
  } else {
    [self showFallbackCardAt:origin fileKind:fileKind];
  }
  return YES;
}

- (void)scheduleFallbackCompletionForGeneration:(uint64_t)generation
                                           after:(CFTimeInterval)delay {
  __weak BPPetView *weakSelf = self;
  dispatch_after(
      dispatch_time(DISPATCH_TIME_NOW,
                    (int64_t)(delay * (CFTimeInterval)NSEC_PER_SEC)),
      dispatch_get_main_queue(), ^{
        BPPetView *strongSelf = weakSelf;
        if (strongSelf == nil ||
            strongSelf->_dropState.generation() != generation) {
          return;
        }
        const PetDropSnapshot completed =
            strongSelf->_dropState.sample(CACurrentMediaTime(),
                                          strongSelf->_reduceMotion);
        if (completed.generation == generation &&
            completed.phase != PetDropPhase::kIdle) {
          // dispatch_after is not expected to fire early, but a short
          // generation-guarded retry keeps the state reusable across clock
          // quantization without touching a newer import.
          [strongSelf
              scheduleFallbackCompletionForGeneration:generation
                                                 after:0.02];
        }
      });
}

- (BOOL)finishImport:(uint64_t)generation result:(uint32_t)result {
  const CFTimeInterval now = CACurrentMediaTime();
  if (!_dropState.can_finish(generation, result, now)) {
    return NO;
  }
  const PetDropSnapshot waiting = _dropState.sample(now, false);
  if (!_dropState.finish(generation, result, now)) {
    return NO;
  }
  _lastRenderedAt = 0.0;
  if (_metalAvailable) {
    if (_animating) {
      [self renderFrame];
    }
    return YES;
  }

  // Without Metal there is no display-link sampling. Latch the current
  // motion policy now and arrange a generation-safe terminal sample so the
  // fallback can accept a later import.
  (void)_dropState.sample(now, _reduceMotion);
  const CFTimeInterval completionDelay =
      _reduceMotion ? 0.15
                    : (result == PET_DROP_REJECTED ? 0.42 : 4.672);
  [self scheduleFallbackCompletionForGeneration:generation
                                          after:completionDelay];

  const NSPoint origin = [self pointForDropOrigin:waiting.origin];
  const NSRect bounds = self.bounds;
  const NSPoint center =
      NSMakePoint(NSWidth(bounds) * _centerUVX,
                  NSHeight(bounds) * _centerUVY);
  [_dropCardLayer removeAllAnimations];
  if (result == PET_DROP_ACCEPTED && _reduceMotion) {
    CABasicAnimation *reduced =
        [CABasicAnimation animationWithKeyPath:@"opacity"];
    reduced.fromValue = @1.0;
    reduced.toValue = @0.0;
    reduced.duration = 0.15;
    [CATransaction begin];
    [CATransaction setDisableActions:YES];
    _dropCardLayer.opacity = 0.0;
    [CATransaction commit];
    [_dropCardLayer addAnimation:reduced forKey:@"pet.drop.reduced"];
    return YES;
  }

  if (result == PET_DROP_ACCEPTED) {
    const NSPoint p1 =
        NSMakePoint(origin.x + (center.x - origin.x) * 0.36,
                    origin.y + (center.y - origin.y) * 0.30);
    const NSPoint p2 =
        NSMakePoint(center.x + (origin.y - center.y) * 0.28,
                    center.y - (origin.x - center.x) * 0.28);
    const NSPoint p3 =
        NSMakePoint(center.x - (origin.x - center.x) * 0.12,
                    center.y - (origin.y - center.y) * 0.12);
    CAKeyframeAnimation *standard =
        [CAKeyframeAnimation animationWithKeyPath:@"position"];
    standard.values = @[
      [NSValue valueWithPoint:origin], [NSValue valueWithPoint:p1],
      [NSValue valueWithPoint:p2], [NSValue valueWithPoint:p3],
      [NSValue valueWithPoint:center], [NSValue valueWithPoint:center]
    ];
    standard.duration = 4.6;
    standard.keyTimes = @[ @0.0, @0.25, @0.55, @0.72, @0.88, @1.0 ];
    standard.calculationMode = kCAAnimationCubic;

    CAKeyframeAnimation *standardOpacity =
        [CAKeyframeAnimation animationWithKeyPath:@"opacity"];
    standardOpacity.values = @[ @1.0, @1.0, @1.0, @0.90, @0.58, @0.0 ];
    standardOpacity.duration = standard.duration;
    standardOpacity.keyTimes = standard.keyTimes;
    [CATransaction begin];
    [CATransaction setDisableActions:YES];
    _dropCardLayer.position = center;
    _dropCardLayer.opacity = 0.0;
    [CATransaction commit];
    [_dropCardLayer addAnimation:standard forKey:@"pet.drop.standard"];
    [_dropCardLayer addAnimation:standardOpacity
                          forKey:@"pet.drop.standard-opacity"];
    return YES;
  }

  if (_reduceMotion) {
    CABasicAnimation *reduced =
        [CABasicAnimation animationWithKeyPath:@"opacity"];
    reduced.fromValue = @1.0;
    reduced.toValue = @0.0;
    reduced.duration = 0.15;
    [CATransaction begin];
    [CATransaction setDisableActions:YES];
    _dropCardLayer.opacity = 0.0;
    [CATransaction commit];
    [_dropCardLayer addAnimation:reduced forKey:@"pet.drop.reduced-reject"];
    return YES;
  }

  const NSPoint outward =
      NSMakePoint(origin.x + (origin.x - center.x) * 0.14,
                  origin.y + (origin.y - center.y) * 0.14);
  CAKeyframeAnimation *rejected =
      [CAKeyframeAnimation animationWithKeyPath:@"position"];
  rejected.values = @[
    [NSValue valueWithPoint:origin], [NSValue valueWithPoint:outward],
    [NSValue valueWithPoint:origin]
  ];
  rejected.keyTimes = @[ @0.0, @0.46, @1.0 ];
  rejected.duration = 0.42;
  CAKeyframeAnimation *rejectedOpacity =
      [CAKeyframeAnimation animationWithKeyPath:@"opacity"];
  rejectedOpacity.values = @[ @1.0, @1.0, @0.0 ];
  rejectedOpacity.keyTimes = rejected.keyTimes;
  rejectedOpacity.duration = rejected.duration;
  [CATransaction begin];
  [CATransaction setDisableActions:YES];
  _dropCardLayer.position = origin;
  _dropCardLayer.opacity = 0.0;
  [CATransaction commit];
  [_dropCardLayer addAnimation:rejected forKey:@"pet.drop.rejected"];
  [_dropCardLayer addAnimation:rejectedOpacity
                        forKey:@"pet.drop.rejected-opacity"];
  return YES;
}

- (void)cancelImport {
  _dropState.cancel();
  _impactState.clear();
  _lastRenderedAt = 0.0;
  [_dropCardLayer removeAllAnimations];
  [_signalLayer removeAllAnimations];
  [CATransaction begin];
  [CATransaction setDisableActions:YES];
  _dropCardLayer.opacity = 0.0;
  _signalLayer.opacity = 0.0;
  [CATransaction commit];
  if (_metalAvailable && _animating) {
    [self renderFrame];
  }
}

- (void)displayLinkTick:(const CVTimeStamp *)outputTime {
  (void)outputTime;
  if (!_frameGate.try_enqueue()) {
    return;
  }
  __weak BPPetView *weakSelf = self;
  dispatch_async(dispatch_get_main_queue(), ^{
    BPPetView *strongSelf = weakSelf;
    if (strongSelf == nil) {
      return;
    }
    strongSelf->_frameGate.complete();
    if (strongSelf->_frameGate.enabled()) {
      [strongSelf renderFrame];
    }
  });
}

- (void)renderFrame {
  if (!_metalAvailable || !_rendererDriver.available() || !_animating) {
    return;
  }
  const CFTimeInterval now = CACurrentMediaTime();
  const PetAnimationSnapshot animation =
      _renderAnimation.sample(now, _reduceMotion);
  const PetDropRenderSnapshot dropRender =
      _dropState.sample_render(now, _reduceMotion);
  const PetDropSnapshot &drop = dropRender.drop;
  if (drop.deliver_once) {
    _impactState.strike(now, drop.origin, drop.file_kind);
  }
  const PetImpactSnapshot impact = _impactState.sample(now);
  const PetRenderActivity activity =
      PetResolveRenderActivity(animation.activity, drop.phase,
                               impact.active);
  const uint32_t targetFps = PetTargetFps(_fps, activity);
  if (targetFps == 0) {
    return;
  }
  const CFTimeInterval interval = 1.0 / (CFTimeInterval)targetFps;
  if (_lastRenderedAt > 0.0 && now - _lastRenderedAt < interval * 0.92) {
    return;
  }
  _lastRenderedAt = now;

  CAMetalLayer *metalLayer =
      [self.layer isKindOfClass:CAMetalLayer.class]
          ? (CAMetalLayer *)self.layer
          : nil;
  if (metalLayer == nil || metalLayer.drawableSize.width <= 0.0 ||
      metalLayer.drawableSize.height <= 0.0) {
    return;
  }
  PetRenderUniforms uniforms = {};
  PetCaptureRegion captureRegion = {};
  captureRegion.panel_extent_uv[0] = 1.0f;
  captureRegion.panel_extent_uv[1] = 1.0f;
  IOSurfaceRef surface =
      self.petHost == nil
          ? nullptr
          : [self.petHost copyLatestSurfaceForView:self
                                           region:&captureRegion];
  uniforms.viewport_px[0] = (float)metalLayer.drawableSize.width;
  uniforms.viewport_px[1] = (float)metalLayer.drawableSize.height;
  uniforms.capture_origin_uv[0] = captureRegion.panel_origin_uv[0];
  uniforms.capture_origin_uv[1] = captureRegion.panel_origin_uv[1];
  uniforms.capture_extent_uv[0] = captureRegion.panel_extent_uv[0];
  uniforms.capture_extent_uv[1] = captureRegion.panel_extent_uv[1];
  uniforms.center_uv[0] = (float)_centerUVX;
  uniforms.center_uv[1] = (float)_centerUVY;
  uniforms.time_seconds = (float)fmod(now - _renderEpoch, 4096.0);
  const PetEffectGeometry effectGeometry =
      PetEffectGeometryForSize(_effectSize);
  uniforms.hole_radius_uv =
      (float)(effectGeometry.shadow_radius / _displayHeight);
  if (_visualStyle == 1) {
    uniforms.temperature = 5200.0f;
    uniforms.inclination = 1.535f;
    uniforms.roll = 0.04f;
    uniforms.disk_inner = 1.9f;
    uniforms.disk_outer = 8.0f;
    uniforms.disk_opacity = 0.88f;
    uniforms.doppler = 0.45f;
    uniforms.beaming = 2.2f;
    uniforms.gain = 2.0f;
    uniforms.contrast = 0.65f;
    uniforms.wind = 7.0f;
    uniforms.speed = 4.0f;
    uniforms.exposure = 1.35f;
    uniforms.stars = 0.0f;
    uniforms.spin = 0.0f;
  } else {
    uniforms.temperature = 8500.0f;
    uniforms.inclination = 1.45f;
    uniforms.roll = 0.15f;
    uniforms.disk_inner = 3.0f;
    uniforms.disk_outer = 9.0f;
    uniforms.disk_opacity = 0.65f;
    uniforms.doppler = 1.0f;
    uniforms.beaming = 3.0f;
    uniforms.gain = 1.0f;
    uniforms.contrast = 0.9f;
    uniforms.wind = 5.0f;
    uniforms.speed = 3.6f;
    uniforms.exposure = 1.0f;
    uniforms.stars = 0.0f;
    uniforms.spin = 0.0f;
  }
  uniforms.spin_phase = 0.0f;
  const bool dropActive =
      drop.phase != PetDropPhase::kIdle;
  const PetDropOrigin renderOrigin =
      dropActive ? drop.origin
                 : (impact.active ? impact.origin
                                  : PetDropOrigin{0.5f, 0.5f});
  uniforms.drop_origin_uv[0] = renderOrigin.x;
  uniforms.drop_origin_uv[1] = renderOrigin.y;
  PetApplyDropMotionUniforms(dropRender, uniforms);
  uniforms.absorption_progress = drop.absorption_progress;
  uniforms.success_progress = animation.success_progress;
  uniforms.error_progress =
      MAX(animation.error_progress, drop.error_progress);
  // Pending jobs stay visible in the main app. The desktop black hole mirrors
  // the upstream renderer and must not spawn unrelated orbiting markers.
  uniforms.pending_count = 0u;
  uniforms.mode =
      PetEffectiveRenderMode(_mode, surface != nullptr);
  uniforms.drop_phase = (uint32_t)drop.phase;
  uniforms.file_kind =
      dropActive ? drop.file_kind : impact.file_kind;
  uniforms.visual_style = _visualStyle;
  uniforms.impact_level = impact.impact_level;
  uniforms.feed_strength = impact.feed_strength;
  const PetRendererStep renderStep =
      _rendererDriver.draw(surface, uniforms);
  if (surface != nullptr) {
    CFRelease(surface);
  }
  if (renderStep == PetRendererStep::kBecameUnavailable) {
    _metalAvailable = NO;
    _frameGate.set_enabled(false);
    if (_displayLink != nullptr) {
      CVDisplayLinkStop(_displayLink);
    }
    [self setNeedsLayout:YES];
    [self.petHost rendererBecameUnavailable];
  }
}

- (void)setPendingCount:(uint32_t)pendingCount {
  if (_visualState.pending_count() == pendingCount) {
    return;
  }
  _visualState.apply_pending_count(pendingCount);

  while (_pendingDotLayers.count > pendingCount) {
    CALayer *dot = _pendingDotLayers.lastObject;
    [dot removeFromSuperlayer];
    [_pendingDotLayers removeLastObject];
  }
  while (_pendingDotLayers.count < pendingCount) {
    CALayer *dot = [CALayer layer];
    dot.backgroundColor =
        [NSColor colorWithSRGBRed:1.0 green:0.66 blue:0.12 alpha:1.0]
            .CGColor;
    dot.shadowColor =
        [NSColor colorWithSRGBRed:1.0 green:0.48 blue:0.05 alpha:1.0]
            .CGColor;
    dot.shadowOpacity = 0.8;
    dot.shadowRadius = 3.0;
    dot.shadowOffset = CGSizeZero;
    [_pendingDotsLayer addSublayer:dot];
    [_pendingDotLayers addObject:dot];
  }
  [self setNeedsLayout:YES];
  [_pendingDotsLayer removeAnimationForKey:@"pet.pending.orbit"];
  if (_animating && !_reduceMotion && pendingCount > 0) {
    CABasicAnimation *orbit =
        [CABasicAnimation animationWithKeyPath:@"transform.rotation.z"];
    orbit.fromValue = @0.0;
    orbit.toValue = @(2.0 * M_PI);
    orbit.duration = 9.0;
    orbit.repeatCount = HUGE_VALF;
    [_pendingDotsLayer addAnimation:orbit forKey:@"pet.pending.orbit"];
  }
}

- (void)setReduceMotion:(BOOL)reduceMotion {
  if (_reduceMotion == reduceMotion) {
    return;
  }
  _reduceMotion = reduceMotion;
  if (reduceMotion || !_animating) {
    [_diskLayer removeAllAnimations];
    [_ringLayer removeAllAnimations];
    [_pendingDotsLayer removeAllAnimations];
    [_signalLayer removeAllAnimations];
  } else {
    [self installAnimations];
  }
}

- (void)setAnimating:(BOOL)animating {
  if (_animating == animating) {
    return;
  }
  _animating = animating;
  if (animating && !_reduceMotion) {
    [self installAnimations];
  } else {
    [_diskLayer removeAllAnimations];
    [_ringLayer removeAllAnimations];
    [_pendingDotsLayer removeAllAnimations];
    [_signalLayer removeAllAnimations];
  }
  _frameGate.set_enabled(animating && _metalAvailable);
  if (_displayLink != nullptr) {
    if (animating && _metalAvailable) {
      _lastRenderedAt = 0.0;
      CVDisplayLinkStart(_displayLink);
      [self renderFrame];
    } else {
      CVDisplayLinkStop(_displayLink);
      _frameGate.complete();
    }
  }
}

- (void)installAnimations {
  if (_reduceMotion || !_animating) {
    return;
  }

  CABasicAnimation *pulse =
      [CABasicAnimation animationWithKeyPath:@"opacity"];
  pulse.fromValue = @0.82;
  pulse.toValue = @1.0;
  pulse.duration = 1.35;
  pulse.autoreverses = YES;
  pulse.repeatCount = HUGE_VALF;
  [_diskLayer addAnimation:pulse forKey:@"pet.pulse"];

  CABasicAnimation *ringPulse =
      [CABasicAnimation animationWithKeyPath:@"opacity"];
  ringPulse.fromValue = @0.62;
  ringPulse.toValue = @1.0;
  ringPulse.duration = 0.85;
  ringPulse.autoreverses = YES;
  ringPulse.repeatCount = HUGE_VALF;
  [_ringLayer addAnimation:ringPulse forKey:@"pet.ring"];

  if (_visualState.pending_dot_count() > 0) {
    CABasicAnimation *orbit =
        [CABasicAnimation animationWithKeyPath:@"transform.rotation.z"];
    orbit.fromValue = @0.0;
    orbit.toValue = @(2.0 * M_PI);
    orbit.duration = 9.0;
    orbit.repeatCount = HUGE_VALF;
    [_pendingDotsLayer addAnimation:orbit forKey:@"pet.pending.orbit"];
  }
}

- (void)pulse {
  if (!_animating) {
    return;
  }
  const PetLitePulseAnimation pulseAnimation =
      PetLitePulseAnimationForMotion(_reduceMotion);
  if (_reduceMotion) {
    CABasicAnimation *fade =
        [CABasicAnimation animationWithKeyPath:@"opacity"];
    fade.fromValue = @0.45;
    fade.toValue = @1.0;
    fade.duration = pulseAnimation.duration_seconds;
    fade.autoreverses = pulseAnimation.autoreverses;
    [_ringLayer addAnimation:fade forKey:@"pet.signal"];
    return;
  }
  CABasicAnimation *signal =
      [CABasicAnimation animationWithKeyPath:@"transform.scale"];
  signal.fromValue = @1.0;
  signal.toValue = @1.08;
  signal.duration = pulseAnimation.duration_seconds;
  signal.autoreverses = pulseAnimation.autoreverses;
  [_ringLayer addAnimation:signal forKey:@"pet.signal"];
}

- (void)signal:(uint32_t)signal {
  if (!_animating) {
    return;
  }
  _visualState.apply_signal(signal);
  _renderAnimation.signal(signal, CACurrentMediaTime());
  _lastRenderedAt = 0.0;
  if (_metalAvailable) {
    [self renderFrame];
    return;
  }
  const PetVisualSignalEffect effect = _visualState.signal_effect();
  if (effect == PetVisualSignalEffect::kNone) {
    return;
  }

  NSColor *color = nil;
  switch (effect) {
    case PetVisualSignalEffect::kImportSwallow:
      color =
          [NSColor colorWithSRGBRed:1.0 green:0.69 blue:0.16 alpha:1.0];
      break;
    case PetVisualSignalEffect::kFailureRedRipple:
      color =
          [NSColor colorWithSRGBRed:1.0 green:0.18 blue:0.20 alpha:1.0];
      break;
    case PetVisualSignalEffect::kSettlementGreenRing:
      color =
          [NSColor colorWithSRGBRed:0.20 green:0.92 blue:0.48 alpha:1.0];
      break;
    case PetVisualSignalEffect::kNone:
      return;
  }

  [CATransaction begin];
  [CATransaction setDisableActions:YES];
  _signalLayer.strokeColor = color.CGColor;
  _signalLayer.fillColor =
      effect == PetVisualSignalEffect::kImportSwallow
          ? [color colorWithAlphaComponent:0.24].CGColor
          : NSColor.clearColor.CGColor;
  _signalLayer.opacity = 0.0;
  [CATransaction commit];
  [_signalLayer removeAllAnimations];

  CABasicAnimation *fade =
      [CABasicAnimation animationWithKeyPath:@"opacity"];
  fade.fromValue = @1.0;
  fade.toValue = @0.0;
  fade.duration =
      PetSignalTransitionDuration(_reduceMotion, 0.42);
  [_signalLayer addAnimation:fade forKey:@"pet.signal.fade"];
  if (_reduceMotion) {
    return;
  }

  switch (effect) {
    case PetVisualSignalEffect::kImportSwallow: {
      CAKeyframeAnimation *swallow =
          [CAKeyframeAnimation animationWithKeyPath:@"transform.scale"];
      swallow.values = @[ @1.0, @1.12, @0.68, @1.0 ];
      swallow.keyTimes = @[ @0.0, @0.22, @0.62, @1.0 ];
      swallow.duration = 0.46;
      [_diskLayer addAnimation:swallow forKey:@"pet.signal.swallow"];
      break;
    }
    case PetVisualSignalEffect::kFailureRedRipple: {
      CABasicAnimation *ripple =
          [CABasicAnimation animationWithKeyPath:@"transform.scale"];
      ripple.fromValue = @0.72;
      ripple.toValue = @1.24;
      ripple.duration = 0.28;
      ripple.repeatCount = 2.0;
      [_signalLayer addAnimation:ripple forKey:@"pet.signal.red-ripple"];
      break;
    }
    case PetVisualSignalEffect::kSettlementGreenRing: {
      CABasicAnimation *greenRing =
          [CABasicAnimation animationWithKeyPath:@"transform.scale"];
      greenRing.fromValue = @0.74;
      greenRing.toValue = @1.14;
      greenRing.duration = 0.48;
      [_signalLayer addAnimation:greenRing
                          forKey:@"pet.signal.green-ring"];
      break;
    }
    case PetVisualSignalEffect::kNone:
      break;
  }
}

@end

@implementation BPCoreHitTargetView {
  BOOL _dragInsideCore;
  PetDropSession _dropSession;
}

- (instancetype)initWithFrame:(NSRect)frame {
  self = [super initWithFrame:frame];
  if (self) {
    [self registerForDraggedTypes:@[ NSPasteboardTypeFileURL ]];
  }
  return self;
}

- (BOOL)pointInsideCore:(NSPoint)point {
  if (self.petHost == nil) {
    return NO;
  }
  const PetEffectGeometry visualGeometry =
      PetEffectGeometryForSize(self.petHost.effectSize);
  const PetEffectGeometry hitGeometry = {
      NSWidth(self.bounds),
      visualGeometry.shadow_radius,
      visualGeometry.hit_radius,
  };
  return PetPointInsideCore(point.x, point.y, hitGeometry);
}

- (NSView *)hitTest:(NSPoint)point {
  return [self pointInsideCore:point] ? [super hitTest:point] : nil;
}

- (void)mouseDown:(NSEvent *)event {
  const NSPoint point = [self convertPoint:event.locationInWindow fromView:nil];
  if (![self pointInsideCore:point]) {
    return;
  }
  [self.petHost beginGestureAt:NSEvent.mouseLocation];
}

- (void)mouseDragged:(NSEvent *)event {
  (void)event;
  [self.petHost continueGestureAt:NSEvent.mouseLocation];
}

- (void)mouseUp:(NSEvent *)event {
  (void)event;
  [self.petHost endGesture];
}

- (BPDropCandidateKind)dropCandidateFromPasteboard:
                           (NSPasteboard *)pasteboard
                                      path:(NSString **)pathOut {
  if (pathOut != nullptr) {
    *pathOut = nil;
  }
  NSArray *urls = [pasteboard
      readObjectsForClasses:@[ NSURL.class ]
                    options:@{
                      NSPasteboardURLReadingFileURLsOnlyKey : @YES
                    }];
  for (id candidate in urls) {
    if (![candidate isKindOfClass:NSURL.class]) {
      continue;
    }
    NSURL *url = (NSURL *)candidate;
    NSString *path = url.path;
    if (!url.fileURL || path == nil || !path.absolutePath) {
      continue;
    }
    const char *fileSystemPath = path.fileSystemRepresentation;
    struct stat status = {};
    if (fileSystemPath == nullptr || lstat(fileSystemPath, &status) != 0 ||
        S_ISLNK(status.st_mode) || !S_ISREG(status.st_mode)) {
      continue;
    }
    NSString *lowercasePath = path.lowercaseString;
    uint32_t fileKind = PET_FILE_NONE;
    if ([lowercasePath hasSuffix:@".gcode.3mf"] ||
        [lowercasePath hasSuffix:@".3mf"]) {
      fileKind = PET_FILE_3MF;
    } else if ([lowercasePath hasSuffix:@".gcode"]) {
      fileKind = PET_FILE_GCODE;
    } else {
      continue;
    }
    if (pathOut != nullptr) {
      *pathOut = path;
    }
    return {YES, fileKind};
  }
  return {NO, PET_FILE_NONE};
}

- (void)cancelDropSessionAndExit {
  _dropSession.cancel();
  if (_dragInsideCore) {
    _dragInsideCore = NO;
    [self.petHost dragExited];
  }
}

- (NSDragOperation)draggingEntered:(id<NSDraggingInfo>)sender {
  const NSPoint point =
      [self convertPoint:sender.draggingLocation fromView:nil];
  NSString *path = nil;
  const BPDropCandidateKind candidate =
      [self dropCandidateFromPasteboard:sender.draggingPasteboard
                                   path:&path];
  if (![self pointInsideCore:point] || !candidate.valid || path == nil) {
    _dropSession.cancel();
    _dragInsideCore = NO;
    return NSDragOperationNone;
  }
  const uint64_t generation =
      _dropSession.enter(path.fileSystemRepresentation, candidate.fileKind);
  if (generation == 0) {
    _dragInsideCore = NO;
    return NSDragOperationNone;
  }
  _dragInsideCore = YES;
  [self.petHost dragEntered];
  return NSDragOperationCopy;
}

- (NSDragOperation)draggingUpdated:(id<NSDraggingInfo>)sender {
  const NSPoint point =
      [self convertPoint:sender.draggingLocation fromView:nil];
  const BOOL insideCore = [self pointInsideCore:point];
  NSString *path = nil;
  const BPDropCandidateKind candidate =
      [self dropCandidateFromPasteboard:sender.draggingPasteboard
                                   path:&path];
  const uint64_t generation = _dropSession.generation();
  const BOOL matches =
      candidate.valid && path != nil &&
      candidate.fileKind == _dropSession.file_kind() &&
      _dropSession.can_submit(generation, path.fileSystemRepresentation,
                              insideCore);
  if (!matches) {
    [self cancelDropSessionAndExit];
    return NSDragOperationNone;
  }
  return NSDragOperationCopy;
}

- (void)draggingExited:(nullable id<NSDraggingInfo>)sender {
  (void)sender;
  [self cancelDropSessionAndExit];
}

- (BOOL)performDragOperation:(id<NSDraggingInfo>)sender {
  const NSPoint point =
      [self convertPoint:sender.draggingLocation fromView:nil];
  const BOOL insideCore = [self pointInsideCore:point];
  NSString *path = nil;
  const BPDropCandidateKind candidate =
      [self dropCandidateFromPasteboard:sender.draggingPasteboard
                                   path:&path];
  const uint64_t generation = _dropSession.generation();
  if (!candidate.valid || path == nil ||
      candidate.fileKind != _dropSession.file_kind() ||
      !_dropSession.can_submit(generation, path.fileSystemRepresentation,
                               insideCore) ||
      self.callback == nullptr || self.window == nil ||
      self.petHost.petView.window == nil) {
    [self cancelDropSessionAndExit];
    return NO;
  }

  const NSPoint hitWindowPoint = [self convertPoint:point toView:nil];
  const NSPoint screenPoint =
      [self.window convertPointToScreen:hitWindowPoint];
  const NSPoint petWindowPoint =
      [self.petHost.petView.window convertPointFromScreen:screenPoint];
  const NSPoint petPoint =
      [self.petHost.petView convertPoint:petWindowPoint fromView:nil];
  if (![self.petHost.petView beginImportWait:generation
                                      origin:petPoint
                                    fileKind:candidate.fileKind]) {
    [self cancelDropSessionAndExit];
    return NO;
  }
  if (!_dropSession.submit(generation, path.fileSystemRepresentation,
                           insideCore)) {
    [self.petHost.petView cancelImport];
    [self cancelDropSessionAndExit];
    return NO;
  }
  self.callback(kPetCallbackFileDropped, path.fileSystemRepresentation,
                0.0, 0.0, generation);
  return YES;
}

- (void)concludeDragOperation:(nullable id<NSDraggingInfo>)sender {
  (void)sender;
  _dragInsideCore = NO;
  _dropSession.cancel();
  [self.petHost dropCompleted];
}

@end

@implementation BPPetHost {
  PetCallback _callback;
  PetWindowLifecycle _windowLifecycle;
  NSString *_metalSource;
  NSMutableArray<BPDisplayPane *> *_panes;
  PetConfig _config;
  BOOL _hasConfig;
  BOOL _sleeping;
  PetDragPersistenceGate _dragPersistence;
}

- (instancetype)initWithCallback:(PetCallback)callback
                      metalSource:(NSString *)metalSource {
  self = [super init];
  if (self) {
    _callback = callback;
    _metalSource = [metalSource copy];
    _panes = [NSMutableArray array];
    _effectSize = 220.0;
    NSScreen *primary = NSScreen.screens.firstObject;
    const NSRect primaryFrame =
        primary == nil ? NSMakeRect(0.0, 0.0, 1440.0, 900.0)
                       : primary.frame;
    _centerScreenPoint =
        NSMakePoint(NSMidX(primaryFrame), NSMidY(primaryFrame));

    const PetEffectGeometry geometry =
        PetEffectGeometryForSize(_effectSize);
    const CGFloat coreHitTargetSide = (CGFloat)(geometry.hit_radius * 2.0);
    const NSRect coreHitTargetFrame =
        NSMakeRect(0.0, 0.0, coreHitTargetSide, coreHitTargetSide);
    _coreHitTargetPanel = [[BPPetPanel alloc]
        initWithContentRect:coreHitTargetFrame
                  styleMask:(NSWindowStyleMaskBorderless |
                             NSWindowStyleMaskNonactivatingPanel)
                    backing:NSBackingStoreBuffered
                      defer:NO];
    _coreHitTargetPanel.opaque = NO;
    _coreHitTargetPanel.backgroundColor = NSColor.clearColor;
    _coreHitTargetPanel.hasShadow = NO;
    const PetWindowPresentation windowPresentation =
        PetWindowPresentationForAlwaysOnTop(false);
    const NSWindowLevel visualWindowLevel =
        PetVisualWindowLevel(windowPresentation);
    _coreHitTargetPanel.level =
        visualWindowLevel + windowPresentation.core_level_offset;
    _coreHitTargetPanel.hidesOnDeactivate = NO;
    _coreHitTargetPanel.releasedWhenClosed = NO;
    _coreHitTargetPanel.restorable = NO;
    _coreHitTargetPanel.ignoresMouseEvents = NO;
    _coreHitTargetPanel.collectionBehavior =
        PetWindowBehavior(windowPresentation);

    _coreHitTargetView =
        [[BPCoreHitTargetView alloc] initWithFrame:coreHitTargetFrame];
    _coreHitTargetView.autoresizingMask =
        NSViewWidthSizable | NSViewHeightSizable;
    _coreHitTargetView.petHost = self;
    _coreHitTargetView.callback = callback;
    _coreHitTargetPanel.contentView = _coreHitTargetView;
    [self rebuildDisplayPanes];
    [self syncCoreHitTargetFrame];
    [NSNotificationCenter.defaultCenter
        addObserver:self
           selector:@selector(screenParametersChanged:)
               name:NSApplicationDidChangeScreenParametersNotification
             object:nil];
    _observingScreenChanges = YES;
    NSNotificationCenter *workspaceCenter =
        NSWorkspace.sharedWorkspace.notificationCenter;
    [workspaceCenter addObserver:self
                        selector:@selector(workspaceWillSleep:)
                            name:NSWorkspaceWillSleepNotification
                          object:nil];
    [workspaceCenter addObserver:self
                        selector:@selector(workspaceDidWake:)
                            name:NSWorkspaceDidWakeNotification
                          object:nil];
    _observingWorkspace = YES;
  }
  return self;
}

- (void)applyConfig:(PetConfig)config {
  _config = config;
  _hasConfig = YES;
  const CGFloat size =
      std::clamp((CGFloat)config.size, kPetMinimumSize, kPetMaximumSize);
  self.effectSize = size;
  const BOOL applyPersistedPosition =
      config.has_position != 0 && !self.gestureActive;
  if (applyPersistedPosition) {
    NSArray<NSScreen *> *screens = NSScreen.screens;
    std::vector<PetScreenFrame> frames;
    frames.reserve(screens.count);
    for (NSScreen *candidate in screens) {
      const NSRect candidateFrame = candidate.frame;
      NSNumber *number =
          candidate.deviceDescription[@"NSScreenNumber"];
      frames.push_back({candidateFrame.origin.x, candidateFrame.origin.y,
                        candidateFrame.size.width, candidateFrame.size.height,
                        candidate.backingScaleFactor,
                        number.unsignedIntValue});
    }
    if (!frames.empty()) {
      const PetScreenPoint recovered = PetRecoverCenter(
          {config.x + size / 2.0, config.y + size / 2.0},
          frames.data(), frames.size());
      self.centerScreenPoint =
          NSMakePoint(recovered.x, recovered.y);
    }
  }
  [self updatePaneGeometry];
  [self syncCoreHitTargetFrame];
  [self updateDisplaySelectionAndEmit:NO];
  for (BPDisplayPane *pane in _panes) {
    [pane.petView setReduceMotion:config.reduce_motion != 0];
    [pane.petView setMode:config.effective_mode];
    [pane.petView setVisualStyle:config.visual_style];
    [pane.petView setFps:config.fps];
    [pane.petView setPendingCount:config.pending_count];
  }

  const PetApplyCapturePlan capturePlan =
      PetApplyCapturePlanForVisibility(config.visible != 0,
                                       config.request_permission != 0);
  if (config.visible != 0) {
    [self showWithPermissionRequest:capturePlan.request_permission];
  } else {
    [self hide];
    if (capturePlan.refresh_capture) {
      [self refreshCaptureWithPermissionRequest:capturePlan.request_permission];
    }
  }
  _config.request_permission = 0;
}

- (void)show {
  [self showWithPermissionRequest:NO];
}

- (void)showWithPermissionRequest:(BOOL)requestPermission {
  if (_windowLifecycle.destroyed()) {
    return;
  }
  _windowLifecycle.show();
  [self syncCoreHitTargetFrame];
  for (BPDisplayPane *pane in _panes) {
    [pane.petView setAnimating:YES];
    [pane.panel orderFrontRegardless];
  }
  [self.coreHitTargetPanel orderFrontRegardless];
  [self refreshCaptureWithPermissionRequest:requestPermission];
}

- (void)hide {
  self.gestureActive = NO;
  _windowLifecycle.hide();
  for (BPDisplayPane *pane in _panes) {
    [pane.petView cancelImport];
    [pane.petView setAnimating:NO];
    mac_capture_stop(pane.captureHandle);
  }
  [self.coreHitTargetPanel orderOut:nil];
  for (BPDisplayPane *pane in _panes) {
    [pane.panel orderOut:nil];
  }
}

- (void)reset {
  NSScreen *screen = NSScreen.screens.firstObject;
  if (screen == nil) {
    return;
  }
  const NSRect frame = screen.frame;
  self.centerScreenPoint =
      NSMakePoint(NSMidX(frame), NSMidY(frame));
  [self updatePaneGeometry];
  [self syncCoreHitTargetFrame];
  [self updateDisplaySelectionAndEmit:NO];
  [self refreshCaptureWithPermissionRequest:NO];
}

- (void)signal:(uint32_t)signal {
  for (BPDisplayPane *pane in _panes) {
    [pane.petView signal:signal];
  }
}

- (void)beginGestureAt:(NSPoint)screenPoint {
  self.gestureActive = YES;
  self.gestureMoved = NO;
  _dragPersistence.begin();
  self.gestureMouseOrigin = screenPoint;
  self.gesturePanelOrigin = self.centerScreenPoint;
}

- (void)continueGestureAt:(NSPoint)screenPoint {
  if (!self.gestureActive) {
    return;
  }
  const NSPoint delta =
      NSMakePoint(screenPoint.x - self.gestureMouseOrigin.x,
                  screenPoint.y - self.gestureMouseOrigin.y);
  if (!self.gestureMoved &&
      hypot(delta.x, delta.y) < kPetDragThreshold) {
    return;
  }
  self.gestureMoved = YES;
  _dragPersistence.mark_dragged();
  self.centerScreenPoint =
      NSMakePoint(self.gesturePanelOrigin.x + delta.x,
                  self.gesturePanelOrigin.y + delta.y);
  [self updatePaneGeometry];
  [self syncCoreHitTargetFrame];
  [self updateDisplaySelectionAndEmit:NO];
}

- (void)endGesture {
  if (!self.gestureActive) {
    return;
  }
  const BOOL moved = self.gestureMoved;
  const BOOL shouldPersist = _dragPersistence.should_persist(true);
  self.gestureActive = NO;
  self.gestureMoved = NO;
  if (!moved) {
    if (_callback != nullptr) {
      _callback(kPetCallbackClicked, nullptr, 0.0, 0.0, self.displayID);
    }
    return;
  }

  [self updateDisplaySelectionAndEmit:NO];
  if (shouldPersist && _callback != nullptr) {
    const NSPoint origin =
        NSMakePoint(self.centerScreenPoint.x - self.effectSize / 2.0,
                    self.centerScreenPoint.y - self.effectSize / 2.0);
    _callback(kPetCallbackMoved, nullptr, origin.x, origin.y, self.displayID);
  }
}

- (void)dragEntered {
  [self.petView setHovering:YES];
  if (_callback != nullptr) {
    _callback(kPetCallbackDropEntered, nullptr, 0.0, 0.0, self.displayID);
  }
}

- (void)dragExited {
  [self.petView setHovering:NO];
  [self.petView cancelImport];
  if (_callback != nullptr) {
    _callback(kPetCallbackDropExited, nullptr, 0.0, 0.0, self.displayID);
  }
}

- (void)dropCompleted {
  [self.petView completeDrop];
}

- (void)rebuildDisplayPanes {
  const BOOL wasVisible =
      _windowLifecycle.visual_visible() && !_windowLifecycle.destroyed();
  for (BPDisplayPane *pane in _panes) {
    [pane.petView cancelImport];
    [pane.petView setAnimating:NO];
    if (pane.captureHandle != nullptr) {
      mac_capture_destroy(pane.captureHandle);
      pane.captureHandle = nullptr;
    }
    pane.petView.petHost = nil;
    [pane.panel orderOut:nil];
    [pane.panel close];
    pane.panel.contentView = nil;
  }
  [_panes removeAllObjects];
  self.panel = nil;
  self.petView = nil;

  for (NSScreen *screen in NSScreen.screens) {
    const NSRect frame = screen.frame;
    NSNumber *screenNumber =
        screen.deviceDescription[@"NSScreenNumber"];
    BPDisplayPane *pane = [[BPDisplayPane alloc] init];
    pane.displayID = screenNumber.unsignedLongLongValue;
    pane.screenFrame = frame;
    pane.captureHandle = mac_capture_create(_callback);
    pane.captureRegion = PetFullDisplayCaptureRegion(
        {frame.origin.x, frame.origin.y, frame.size.width, frame.size.height,
         screen.backingScaleFactor, screenNumber.unsignedIntValue});

    pane.panel = [[BPPetPanel alloc]
        initWithContentRect:frame
                  styleMask:(NSWindowStyleMaskBorderless |
                             NSWindowStyleMaskNonactivatingPanel)
                    backing:NSBackingStoreBuffered
                      defer:NO];
    pane.panel.opaque = NO;
    pane.panel.backgroundColor = NSColor.clearColor;
    pane.panel.hasShadow = NO;
    const PetWindowPresentation windowPresentation =
        PetWindowPresentationForAlwaysOnTop(false);
    pane.panel.level = PetVisualWindowLevel(windowPresentation);
    pane.panel.hidesOnDeactivate = NO;
    pane.panel.releasedWhenClosed = NO;
    pane.panel.restorable = NO;
    pane.panel.ignoresMouseEvents = YES;
    pane.panel.collectionBehavior =
        PetWindowBehavior(windowPresentation);

    pane.petView =
        [[BPPetView alloc] initWithFrame:NSMakeRect(
                                         0.0, 0.0, NSWidth(frame),
                                         NSHeight(frame))
                              metalSource:_metalSource];
    pane.petView.petHost = self;
    pane.petView.autoresizingMask =
        NSViewWidthSizable | NSViewHeightSizable;
    pane.panel.contentView = pane.petView;
    [_panes addObject:pane];

    if (_hasConfig) {
      [pane.petView setReduceMotion:_config.reduce_motion != 0];
      [pane.petView setMode:_config.effective_mode];
      [pane.petView setVisualStyle:_config.visual_style];
      [pane.petView setFps:_config.fps];
      [pane.petView setPendingCount:_config.pending_count];
    }
    if (wasVisible) {
      [pane.petView setAnimating:YES];
      [pane.panel orderFrontRegardless];
    }
  }
  [self updatePaneGeometry];
  [self updateDisplaySelectionAndEmit:NO];
  if (wasVisible) {
    [self.coreHitTargetPanel orderFrontRegardless];
  }
}

- (void)updatePaneGeometry {
  for (BPDisplayPane *pane in _panes) {
    const PetScreenPoint centerUV = PetCenterUVForDisplay(
        {self.centerScreenPoint.x, self.centerScreenPoint.y},
        {pane.screenFrame.origin.x, pane.screenFrame.origin.y,
         pane.screenFrame.size.width, pane.screenFrame.size.height, 1.0,
         (uint32_t)pane.displayID});
    [pane.petView setCenterUVX:centerUV.x
                            y:centerUV.y
                   effectSize:self.effectSize
                displayHeight:NSHeight(pane.screenFrame)];
  }
}

- (void)syncCoreHitTargetFrame {
  const PetEffectGeometry geometry =
      PetEffectGeometryForSize(self.effectSize);
  const CGFloat side = (CGFloat)(geometry.hit_radius * 2.0);
  const NSRect coreHitTargetFrame =
      NSMakeRect(self.centerScreenPoint.x - side / 2.0,
                 self.centerScreenPoint.y - side / 2.0, side, side);
  [self.coreHitTargetPanel setFrame:coreHitTargetFrame display:NO];
}

- (NSScreen *)screenForPanel {
  NSArray<NSScreen *> *screens = NSScreen.screens;
  if (screens.count == 0) {
    return nil;
  }
  std::vector<PetScreenFrame> frames;
  frames.reserve(screens.count);
  for (NSScreen *screen in screens) {
    const NSRect screenFrame = screen.frame;
    NSNumber *screenNumber = screen.deviceDescription[@"NSScreenNumber"];
    frames.push_back({screenFrame.origin.x, screenFrame.origin.y,
                      screenFrame.size.width, screenFrame.size.height,
                      screen.backingScaleFactor,
                      screenNumber.unsignedIntValue});
  }
  size_t currentIndex = 0;
  for (size_t index = 0; index < frames.size(); ++index) {
    if (frames[index].display_id == self.displayID) {
      currentIndex = index;
      break;
    }
  }
  const size_t selected = PetDisplayIndexForPoint(
      {self.centerScreenPoint.x, self.centerScreenPoint.y},
      frames.data(), frames.size(), currentIndex);
  return screens[selected];
}

- (void)updateDisplaySelectionAndEmit:(BOOL)emit {
  NSScreen *screen = [self screenForPanel];
  if (screen == nil) {
    return;
  }
  NSNumber *screenNumber = screen.deviceDescription[@"NSScreenNumber"];
  const uint64_t displayID = screenNumber.unsignedLongLongValue;
  if (displayID == self.displayID) {
    for (BPDisplayPane *pane in _panes) {
      if (pane.displayID == displayID) {
        self.panel = pane.panel;
        self.petView = pane.petView;
        break;
      }
    }
    return;
  }
  self.displayID = displayID;
  for (BPDisplayPane *pane in _panes) {
    if (pane.displayID == displayID) {
      self.panel = pane.panel;
      self.petView = pane.petView;
      break;
    }
  }
  if (emit && _callback != nullptr) {
    const NSPoint origin =
        NSMakePoint(self.centerScreenPoint.x - self.effectSize / 2.0,
                    self.centerScreenPoint.y - self.effectSize / 2.0);
    _callback(kPetCallbackDisplayChanged, nullptr, origin.x, origin.y,
              displayID);
  }
}

- (void)screenParametersChanged:(NSNotification *)notification {
  (void)notification;
  if (_windowLifecycle.destroyed()) {
    return;
  }
  NSArray<NSScreen *> *screens = NSScreen.screens;
  if (screens.count == 0) {
    for (BPDisplayPane *pane in _panes) {
      mac_capture_stop(pane.captureHandle);
    }
    return;
  }
  std::vector<PetScreenFrame> frames;
  frames.reserve(screens.count);
  for (NSScreen *screen in screens) {
    const NSRect frame = screen.frame;
    NSNumber *screenNumber =
        screen.deviceDescription[@"NSScreenNumber"];
    frames.push_back(
        {frame.origin.x, frame.origin.y, frame.size.width, frame.size.height,
         screen.backingScaleFactor, screenNumber.unsignedIntValue});
  }
  const PetScreenPoint recovered = PetRecoverCenter(
      {self.centerScreenPoint.x, self.centerScreenPoint.y},
      frames.data(), frames.size());
  self.centerScreenPoint = NSMakePoint(recovered.x, recovered.y);
  [self rebuildDisplayPanes];
  [self syncCoreHitTargetFrame];
  [self updateDisplaySelectionAndEmit:NO];
  [self refreshCaptureWithPermissionRequest:NO];
  if (_callback != nullptr) {
    const NSPoint origin =
        NSMakePoint(self.centerScreenPoint.x - self.effectSize / 2.0,
                    self.centerScreenPoint.y - self.effectSize / 2.0);
    _callback(kPetCallbackDisplayChanged, nullptr, origin.x,
              origin.y, self.displayID);
  }
}

- (void)workspaceWillSleep:(NSNotification *)notification {
  (void)notification;
  if (_windowLifecycle.destroyed() || _sleeping) {
    return;
  }
  _sleeping = YES;
  for (BPDisplayPane *pane in _panes) {
    mac_capture_stop(pane.captureHandle);
    [pane.petView cancelImport];
    [pane.petView setAnimating:NO];
  }
  if (_callback != nullptr) {
    _callback(kPetCallbackSleep, nullptr, 0.0, 0.0, self.displayID);
  }
}

- (void)workspaceDidWake:(NSNotification *)notification {
  (void)notification;
  if (_windowLifecycle.destroyed() || !_sleeping) {
    return;
  }
  _sleeping = NO;
  // Screen enumeration and clamping precede the permission recheck and any
  // capture restart performed by refreshCaptureWithPermissionRequest:.
  [self screenParametersChanged:nil];
  if (_hasConfig && _windowLifecycle.visual_visible()) {
    for (BPDisplayPane *pane in _panes) {
      [pane.petView setAnimating:YES];
    }
  }
  if (_callback != nullptr) {
    _callback(kPetCallbackWake, nullptr, 0.0, 0.0, self.displayID);
  }
}

- (void)refreshCaptureWithPermissionRequest:(BOOL)requestPermission {
  if (!_hasConfig) {
    return;
  }
  const BOOL visible = _windowLifecycle.visual_visible();
  for (BPDisplayPane *pane in _panes) {
    if (pane.captureHandle == nullptr) {
      continue;
    }
    const PetRendererDecision renderer =
        PetRendererDecisionForMetalAvailability(
            pane.petView.metalAvailable);
    const BOOL captureVisible =
        visible && !_sleeping && renderer.real_effect_available &&
        !renderer.stop_capture;
    const PetCaptureRegion emptyRegion = {};
    if (!captureVisible || _config.mode != 0) {
      mac_capture_configure(pane.captureHandle, emptyRegion,
                            _config.mode == 0, captureVisible,
                            requestPermission, _config.fps);
      continue;
    }
    mac_capture_configure(pane.captureHandle, pane.captureRegion, true, true,
                          requestPermission, _config.fps);
  }
}

- (void)rendererBecameUnavailable {
  for (BPDisplayPane *pane in _panes) {
    if (!pane.petView.metalAvailable) {
      mac_capture_stop(pane.captureHandle);
    }
  }
  if (_callback != nullptr) {
    _callback(kPetCallbackCaptureFailed, "metal_unavailable", 0.0, 0.0,
              self.displayID);
  }
}

- (IOSurfaceRef)copyLatestSurfaceForView:(BPPetView *)view
                                  region:(PetCaptureRegion *)regionOut {
  for (BPDisplayPane *pane in _panes) {
    if (pane.petView == view) {
      return mac_capture_copy_latest_surface(pane.captureHandle, regionOut);
    }
  }
  if (regionOut != nullptr) {
    *regionOut = {};
    regionOut->panel_extent_uv[0] = 1.0f;
    regionOut->panel_extent_uv[1] = 1.0f;
  }
  return nullptr;
}

- (uint32_t)captureState {
  uint32_t state = PET_CAPTURE_UNAVAILABLE;
  for (BPDisplayPane *pane in _panes) {
    const uint32_t paneState = mac_capture_state(pane.captureHandle);
    if (paneState == PET_CAPTURE_FAILED) {
      return paneState;
    }
    if (paneState == PET_CAPTURE_READY) {
      state = paneState;
    } else if (state != PET_CAPTURE_READY) {
      state = paneState;
    }
  }
  return state;
}

- (uint32_t)rendererState {
  for (BPDisplayPane *pane in _panes) {
    if (!pane.petView.metalAvailable) {
      return PET_RENDERER_UNAVAILABLE;
    }
  }
  return _panes.count == 0 ? PET_RENDERER_UNAVAILABLE
                           : PET_RENDERER_READY;
}

- (uint32_t)shutdown {
  uint32_t shutdownState = PET_SHUTDOWN_COMPLETE;
  for (BPDisplayPane *pane in _panes) {
    [pane.petView cancelImport];
    [pane.petView setAnimating:NO];
    if (pane.captureHandle != nullptr) {
      const uint32_t paneShutdown =
          mac_capture_destroy(pane.captureHandle);
      if (paneShutdown != PET_SHUTDOWN_COMPLETE) {
        shutdownState = paneShutdown;
      }
      pane.captureHandle = nullptr;
    }
  }
  _windowLifecycle.destroy();
  if (self.observingScreenChanges) {
    [NSNotificationCenter.defaultCenter removeObserver:self];
    self.observingScreenChanges = NO;
  }
  if (self.observingWorkspace) {
    [NSWorkspace.sharedWorkspace.notificationCenter removeObserver:self];
    self.observingWorkspace = NO;
  }
  [self.coreHitTargetView unregisterDraggedTypes];
  self.coreHitTargetView.petHost = nil;
  self.coreHitTargetView.callback = nullptr;
  [self.coreHitTargetPanel close];
  self.coreHitTargetPanel.contentView = nil;
  for (BPDisplayPane *pane in _panes) {
    pane.petView.petHost = nil;
    [pane.panel close];
    pane.panel.contentView = nil;
  }
  [_panes removeAllObjects];
  self.coreHitTargetView = nil;
  self.coreHitTargetPanel = nil;
  self.petView = nil;
  self.panel = nil;
  _callback = nullptr;
  return shutdownState;
}

@end

@implementation BPPetBridge {
  PetApplyGenerationGate _applyGeneration;
}

- (instancetype)initWithCallback:(PetCallback)callback
                      metalSource:(NSString *)metalSource {
  self = [super init];
  if (self) {
    _callback = callback;
    _metalSource = [metalSource copy];
  }
  return self;
}

- (void)ensureHost {
  if (!self.destroyed && self.host == nil) {
    self.host = [[BPPetHost alloc] initWithCallback:self.callback
                                        metalSource:self.metalSource];
  }
}

- (uint64_t)issueApplyGeneration {
  return _applyGeneration.issue();
}

- (BOOL)acceptApplyGeneration:(uint64_t)generation {
  return _applyGeneration.accept(generation);
}

- (uint32_t)shutdown {
  if (self.destroyed) {
    return self.shutdownState;
  }
  self.destroyed = YES;
  self.shutdownState =
      self.host == nil ? PET_SHUTDOWN_COMPLETE : [self.host shutdown];
  self.host = nil;
  self.metalSource = nil;
  self.callback = nullptr;
  return self.shutdownState;
}

@end

static void RunOnMain(dispatch_block_t block) {
  if (NSThread.isMainThread) {
    block();
  } else {
    dispatch_async(dispatch_get_main_queue(), block);
  }
}

static void RunOnMainAndWait(dispatch_block_t block) {
  if (NSThread.isMainThread) {
    block();
  } else if (NSApp == nil) {
    // Cargo's native ABI tests have no NSApplication or serviced main
    // dispatch queue. Preserve the production synchronous path while keeping
    // that headless construction/destruction check non-blocking.
    dispatch_async(dispatch_get_main_queue(), block);
  } else {
    dispatch_sync(dispatch_get_main_queue(), block);
  }
}

static void RunOnMainForShutdownAndWait(dispatch_block_t block) {
  if (NSThread.isMainThread || NSApp == nil) {
    block();
  } else {
    dispatch_sync(dispatch_get_main_queue(), block);
  }
}

static bool IsValidConfig(PetConfig config) {
  const bool validFps =
      config.fps == 0 || config.fps == 30 || config.fps == 60;
  return config.abi_version == kPetAbiVersion && config.mode <= 1 &&
         config.effective_mode <= 1 &&
         config.has_position <= 1 &&
         isfinite(config.size) && config.size >= kPetMinimumSize &&
         config.size <= kPetMaximumSize &&
         (!config.has_position || (isfinite(config.x) && isfinite(config.y))) &&
         validFps && config.visible <= 1 &&
         config.reduce_motion <= 1 && config.request_permission <= 1 &&
         PetVisualStyleIsValid(config.visual_style);
}

extern "C" void *pet_create(PetCallback callback,
                             const char *metal_source) {
  NSString *source =
      metal_source == nullptr
          ? @""
          : [[NSString alloc] initWithUTF8String:metal_source];
  if (source == nil) {
    return nullptr;
  }
  BPPetBridge *bridge = [[BPPetBridge alloc] initWithCallback:callback
                                                  metalSource:source];
  void *handle = (__bridge_retained void *)bridge;
  RunOnMain(^{
    [bridge ensureHost];
  });
  return handle;
}

extern "C" uint32_t pet_destroy(void *handle) {
  if (handle == nullptr) {
    return PET_SHUTDOWN_COMPLETE;
  }
  BPPetBridge *bridge = (__bridge_transfer BPPetBridge *)handle;
  __block uint32_t shutdownState = PET_SHUTDOWN_COMPLETE;
  RunOnMainForShutdownAndWait(^{
    shutdownState = [bridge shutdown];
  });
  return shutdownState;
}

extern "C" bool pet_apply(void *handle, PetConfig config) {
  if (handle == nullptr || !IsValidConfig(config)) {
    return false;
  }
  BPPetBridge *bridge = (__bridge BPPetBridge *)handle;
  const uint64_t generation = [bridge issueApplyGeneration];
  // Routine apply is deliberately asynchronous. Rust callers may hold the
  // runtime-state mutex while submitting it. Main-thread calls can run
  // inline while an older worker call is still queued, so the generation
  // gate rejects any stale config that arrives out of submission order.
  RunOnMain(^{
    if (!bridge.destroyed &&
        [bridge acceptApplyGeneration:generation]) {
      [bridge ensureHost];
      [bridge.host applyConfig:config];
    }
  });
  return true;
}

extern "C" void pet_show(void *handle) {
  if (handle == nullptr) {
    return;
  }
  BPPetBridge *bridge = (__bridge BPPetBridge *)handle;
  RunOnMain(^{
    if (!bridge.destroyed) {
      [bridge ensureHost];
      [bridge.host show];
    }
  });
}

extern "C" void pet_hide(void *handle) {
  if (handle == nullptr) {
    return;
  }
  BPPetBridge *bridge = (__bridge BPPetBridge *)handle;
  RunOnMain(^{
    if (!bridge.destroyed) {
      [bridge.host hide];
    }
  });
}

extern "C" void pet_reset(void *handle) {
  if (handle == nullptr) {
    return;
  }
  BPPetBridge *bridge = (__bridge BPPetBridge *)handle;
  RunOnMain(^{
    if (!bridge.destroyed) {
      [bridge ensureHost];
      [bridge.host reset];
    }
  });
}

extern "C" void pet_signal(void *handle, uint32_t signal) {
  if (handle == nullptr) {
    return;
  }
  BPPetBridge *bridge = (__bridge BPPetBridge *)handle;
  RunOnMain(^{
    if (!bridge.destroyed) {
      [bridge ensureHost];
      [bridge.host signal:signal];
    }
  });
}

extern "C" void pet_finish_drop(void *handle, uint64_t generation,
                                 uint32_t result) {
  if (handle == nullptr || generation == 0 ||
      (result != PET_DROP_ACCEPTED && result != PET_DROP_REJECTED)) {
    return;
  }
  BPPetBridge *bridge = (__bridge BPPetBridge *)handle;
  RunOnMain(^{
    if (!bridge.destroyed && bridge.host != nil) {
      [bridge.host.petView finishImport:generation result:result];
    }
  });
}

extern "C" uint32_t pet_capture_state(void *handle) {
  if (handle == nullptr) {
    return PET_CAPTURE_UNAVAILABLE;
  }
  if (!NSThread.isMainThread && NSApp == nil) {
    return PET_CAPTURE_UNAVAILABLE;
  }
  BPPetBridge *bridge = (__bridge BPPetBridge *)handle;
  __block uint32_t state = PET_CAPTURE_UNAVAILABLE;
  RunOnMainAndWait(^{
    if (!bridge.destroyed) {
      [bridge ensureHost];
      state = [bridge.host captureState];
    }
  });
  return state;
}

extern "C" uint32_t pet_renderer_state(void *handle) {
  if (handle == nullptr) {
    return PET_RENDERER_UNAVAILABLE;
  }
  if (!NSThread.isMainThread && NSApp == nil) {
    return PET_RENDERER_UNAVAILABLE;
  }
  BPPetBridge *bridge = (__bridge BPPetBridge *)handle;
  __block uint32_t state = PET_RENDERER_UNAVAILABLE;
  RunOnMainAndWait(^{
    if (!bridge.destroyed) {
      [bridge ensureHost];
      state = [bridge.host rendererState];
    }
  });
  return state;
}

extern "C" uint32_t pet_abi_version(void) {
  return kPetAbiVersion;
}

#pragma clang diagnostic pop
