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
static const CGFloat kPetSafeInset = 16.0;

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
- (void)setPendingCount:(uint32_t)pendingCount;
- (void)signal:(uint32_t)signal;
- (void)pulse;
- (void)displayLinkTick:(const CVTimeStamp *)outputTime;
- (void)renderFrame;
- (void)updateDrawableSize;
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
@property(nonatomic, assign) BOOL gestureActive;
@property(nonatomic, assign) NSPoint gestureMouseOrigin;
@property(nonatomic, assign) NSPoint gesturePanelOrigin;
@property(nonatomic, assign) BOOL gestureMoved;
@property(nonatomic, assign) uint64_t displayID;
@property(nonatomic, assign) PetCaptureRegion captureRegion;
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
- (NSScreen *)screenForPanel;
- (void)updateDisplaySelectionAndEmit:(BOOL)emit;
- (void)screenParametersChanged:(NSNotification *)notification;
- (void)workspaceWillSleep:(NSNotification *)notification;
- (void)workspaceDidWake:(NSNotification *)notification;
- (void)refreshCaptureWithPermissionRequest:(BOOL)requestPermission;
- (void)rendererBecameUnavailable;
- (IOSurfaceRef)copyLatestSurface CF_RETURNS_RETAINED;
- (uint32_t)captureState;
- (uint32_t)rendererState;
- (uint32_t)shutdown;
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
  PetRenderAnimationState _renderAnimation;
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

    if (CVDisplayLinkCreateWithActiveCGDisplays(&_displayLink) ==
        kCVReturnSuccess) {
      CVDisplayLinkSetOutputCallback(_displayLink, PetDisplayLinkCallback,
                                     (__bridge void *)self);
    }
  }
  return self;
}

- (void)dealloc {
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
  const CGFloat panelSide =
      MIN(CGRectGetWidth(bounds), CGRectGetHeight(bounds));
  const PetEffectGeometry geometry =
      PetEffectGeometryForSize(panelSide);
  const CGFloat ringWidth = MAX(3.0, panelSide * 0.035);
  const CGRect effectFrame =
      CGRectMake(CGRectGetMidX(bounds) -
                     geometry.panel_side / 2.0,
                 CGRectGetMidY(bounds) -
                     geometry.panel_side / 2.0,
                 geometry.panel_side,
                 geometry.panel_side);
  const CGRect shadowFrame =
      CGRectMake(CGRectGetMidX(bounds) -
                     geometry.shadow_radius,
                 CGRectGetMidY(bounds) -
                     geometry.shadow_radius,
                 geometry.shadow_radius * 2.0,
                 geometry.shadow_radius * 2.0);

  [CATransaction begin];
  [CATransaction setDisableActions:YES];
  _diskLayer.hidden = _metalAvailable;
  _ringLayer.hidden = _metalAvailable;
  _pendingDotsLayer.hidden = _metalAvailable;
  _signalLayer.hidden = _metalAvailable;
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
        CGRectGetMidX(bounds) + cos(placement.angle_radians) * orbitRadius;
    const CGFloat centerY =
        CGRectGetMidY(bounds) + sin(placement.angle_radians) * orbitRadius;
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
  const CGFloat visualSide =
      MIN(CGRectGetWidth(bounds), CGRectGetHeight(bounds));
  const CGFloat drawableLogicalSide =
      (CGFloat)PetDrawableLogicalSide(visualSide);
  const PetDrawableMetrics metrics = PetDrawableMetricsForLogicalSize(
      drawableLogicalSide, drawableLogicalSide, scale);
  metalLayer.contentsScale = metrics.contents_scale;
  metalLayer.drawableSize =
      CGSizeMake(metrics.pixel_width, metrics.pixel_height);
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
  const uint32_t targetFps = PetTargetFps(_fps, animation.activity);
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
  IOSurfaceRef surface =
      self.petHost == nil ? nullptr : [self.petHost copyLatestSurface];
  PetCaptureRegion captureRegion = {};
  captureRegion.panel_extent_uv[0] = 1.0f;
  captureRegion.panel_extent_uv[1] = 1.0f;
  if (_mode == 0 && surface != nullptr && self.petHost != nil) {
    const PetCaptureRegion configuredRegion = self.petHost.captureRegion;
    if (configuredRegion.source_width > 0.0 &&
        configuredRegion.source_height > 0.0) {
      captureRegion = configuredRegion;
    }
  }
  uniforms.viewport_px[0] = (float)metalLayer.drawableSize.width;
  uniforms.viewport_px[1] = (float)metalLayer.drawableSize.height;
  uniforms.capture_origin_uv[0] = captureRegion.panel_origin_uv[0];
  uniforms.capture_origin_uv[1] = captureRegion.panel_origin_uv[1];
  uniforms.capture_extent_uv[0] = captureRegion.panel_extent_uv[0];
  uniforms.capture_extent_uv[1] = captureRegion.panel_extent_uv[1];
  uniforms.time_seconds = (float)fmod(now - _renderEpoch, 4096.0);
  uniforms.hole_radius_uv = 0.075f;
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
    uniforms.temperature = 4500.0f;
    uniforms.inclination = 1.52f;
    uniforms.roll = 0.10f;
    uniforms.disk_inner = 2.2f;
    uniforms.disk_outer = 7.0f;
    uniforms.disk_opacity = 0.85f;
    uniforms.doppler = 0.35f;
    uniforms.beaming = 2.0f;
    uniforms.gain = 1.4f;
    uniforms.contrast = 0.5f;
    uniforms.wind = 7.0f;
    uniforms.speed = 5.0f;
    uniforms.exposure = 1.20f;
    uniforms.stars = 0.0f;
    uniforms.spin = 0.0f;
  }
  uniforms.spin_phase = 0.0f;
  uniforms.drop_origin_uv[0] = 0.5f;
  uniforms.drop_origin_uv[1] = 0.5f;
  uniforms.drop_progress = animation.hover_progress;
  uniforms.absorption_progress = animation.swallow_progress;
  uniforms.success_progress = animation.success_progress;
  uniforms.error_progress = animation.error_progress;
  uniforms.pending_count = _visualState.pending_count();
  uniforms.mode = _mode;
  uniforms.reduce_motion = _reduceMotion ? 1 : 0;
  uniforms.drop_phase = 0;
  uniforms.file_kind = 0;
  uniforms.visual_style = _visualStyle;
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
      PetEffectGeometryForSize(NSWidth(self.petHost.panel.frame));
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

- (NSDragOperation)draggingEntered:(id<NSDraggingInfo>)sender {
  const NSPoint point =
      [self convertPoint:sender.draggingLocation fromView:nil];
  if (![self pointInsideCore:point]) {
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
  if (insideCore != _dragInsideCore) {
    _dragInsideCore = insideCore;
    if (insideCore) {
      [self.petHost dragEntered];
    } else {
      [self.petHost dragExited];
    }
  }
  return insideCore ? NSDragOperationCopy : NSDragOperationNone;
}

- (void)draggingExited:(nullable id<NSDraggingInfo>)sender {
  (void)sender;
  if (_dragInsideCore) {
    _dragInsideCore = NO;
    [self.petHost dragExited];
  }
}

- (BOOL)performDragOperation:(id<NSDraggingInfo>)sender {
  const NSPoint point =
      [self convertPoint:sender.draggingLocation fromView:nil];
  if (![self pointInsideCore:point]) {
    return NO;
  }
  NSPasteboard *pasteboard = sender.draggingPasteboard;
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
    NSNumber *isRegularFile = nil;
    if (![url getResourceValue:&isRegularFile
                        forKey:NSURLIsRegularFileKey
                         error:nil] ||
        !isRegularFile.boolValue) {
      continue;
    }
    NSString *lowercasePath = path.lowercaseString;
    if (![lowercasePath hasSuffix:@".gcode.3mf"] &&
        ![lowercasePath hasSuffix:@".3mf"] &&
        ![lowercasePath hasSuffix:@".gcode"]) {
      continue;
    }
    if (self.callback != nullptr) {
      self.callback(kPetCallbackFileDropped, path.fileSystemRepresentation,
                    0.0, 0.0, 0);
    }
    return YES;
  }
  return NO;
}

- (void)concludeDragOperation:(nullable id<NSDraggingInfo>)sender {
  (void)sender;
  _dragInsideCore = NO;
  [self.petHost dropCompleted];
}

@end

@implementation BPPetHost {
  PetCallback _callback;
  PetWindowLifecycle _windowLifecycle;
  void *_captureHandle;
  PetConfig _config;
  BOOL _hasConfig;
  BOOL _sleeping;
  PetDragPersistenceGate _dragPersistence;
  PetCaptureConfigurationGate _captureConfiguration;
}

- (instancetype)initWithCallback:(PetCallback)callback
                      metalSource:(NSString *)metalSource {
  self = [super init];
  if (self) {
    _callback = callback;
    _captureHandle = mac_capture_create(callback);
    const NSRect frame = NSMakeRect(0.0, 0.0, 220.0, 220.0);
    _panel = [[BPPetPanel alloc]
        initWithContentRect:frame
                  styleMask:(NSWindowStyleMaskBorderless |
                             NSWindowStyleMaskNonactivatingPanel)
                    backing:NSBackingStoreBuffered
                      defer:NO];
    _panel.opaque = NO;
    _panel.backgroundColor = NSColor.clearColor;
    _panel.hasShadow = NO;
    _panel.level = NSFloatingWindowLevel;
    _panel.hidesOnDeactivate = NO;
    _panel.releasedWhenClosed = NO;
    _panel.restorable = NO;
    // The lens/accretion effect never participates in hit testing.
    _panel.ignoresMouseEvents = YES;
    _panel.collectionBehavior =
        NSWindowCollectionBehaviorCanJoinAllSpaces |
        NSWindowCollectionBehaviorFullScreenAuxiliary;

    _petView = [[BPPetView alloc] initWithFrame:frame
                                    metalSource:metalSource];
    _petView.petHost = self;
    _petView.autoresizingMask = NSViewWidthSizable | NSViewHeightSizable;
    _panel.contentView = _petView;

    const PetEffectGeometry geometry = PetEffectGeometryForSize(220.0);
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
    _coreHitTargetPanel.level = NSFloatingWindowLevel;
    _coreHitTargetPanel.hidesOnDeactivate = NO;
    _coreHitTargetPanel.releasedWhenClosed = NO;
    _coreHitTargetPanel.restorable = NO;
    _coreHitTargetPanel.ignoresMouseEvents = NO;
    _coreHitTargetPanel.collectionBehavior =
        NSWindowCollectionBehaviorCanJoinAllSpaces |
        NSWindowCollectionBehaviorFullScreenAuxiliary;

    _coreHitTargetView =
        [[BPCoreHitTargetView alloc] initWithFrame:coreHitTargetFrame];
    _coreHitTargetView.autoresizingMask =
        NSViewWidthSizable | NSViewHeightSizable;
    _coreHitTargetView.petHost = self;
    _coreHitTargetView.callback = callback;
    _coreHitTargetPanel.contentView = _coreHitTargetView;
    [_panel addChildWindow:_coreHitTargetPanel ordered:NSWindowAbove];
    [self reset];
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
  const NSRect oldFrame = self.panel.frame;
  const NSPoint center = NSMakePoint(NSMidX(oldFrame), NSMidY(oldFrame));
  const CGFloat size = (CGFloat)config.size;
  const BOOL applyPersistedPosition =
      config.has_position != 0 && !self.gestureActive;
  NSRect frame = NSMakeRect(
      applyPersistedPosition ? config.x : center.x - size / 2.0,
      applyPersistedPosition ? config.y : center.y - size / 2.0, size, size);
  NSScreen *screen = nil;
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
      const size_t selected = PetSavedDisplayOrPrimaryIndex(
          config.display_id, frames.data(), frames.size());
      screen = screens[selected];
    }
  } else {
    screen = [self screenForPanel];
  }
  if (screen != nil) {
    const NSRect safeFrame = screen.visibleFrame;
    const PetPanelFrame clamped = PetClampPanelToDisplay(
        {frame.origin.x, frame.origin.y, frame.size.width, frame.size.height},
        {safeFrame.origin.x, safeFrame.origin.y, safeFrame.size.width,
         safeFrame.size.height, screen.backingScaleFactor, 0},
        kPetSafeInset);
    frame.origin = NSMakePoint(clamped.x, clamped.y);
  }
  [self.panel setFrame:frame display:YES animate:NO];
  [self syncCoreHitTargetFrame];
  [self updateDisplaySelectionAndEmit:NO];
  [self.petView setReduceMotion:config.reduce_motion != 0];
  [self.petView setMode:config.effective_mode];
  [self.petView setVisualStyle:config.visual_style];
  [self.petView setFps:config.fps];
  [self.petView setPendingCount:config.pending_count];

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
  [self.petView setAnimating:YES];
  [self.panel orderFrontRegardless];
  [self.coreHitTargetPanel orderFrontRegardless];
  [self refreshCaptureWithPermissionRequest:requestPermission];
}

- (void)hide {
  self.gestureActive = NO;
  _windowLifecycle.hide();
  [self.petView setAnimating:NO];
  mac_capture_stop(_captureHandle);
  _captureConfiguration.invalidate();
  [self.coreHitTargetPanel orderOut:nil];
  [self.panel orderOut:nil];
}

- (void)reset {
  NSScreen *screen = NSScreen.screens.firstObject;
  if (screen == nil) {
    return;
  }
  const NSRect visibleFrame = screen.visibleFrame;
  NSRect frame = self.panel.frame;
  frame.origin =
      NSMakePoint(NSMidX(visibleFrame) - NSWidth(frame) / 2.0,
                  NSMidY(visibleFrame) - NSHeight(frame) / 2.0);
  [self.panel setFrame:frame display:NO];
  [self syncCoreHitTargetFrame];
  [self updateDisplaySelectionAndEmit:NO];
  [self refreshCaptureWithPermissionRequest:NO];
}

- (void)signal:(uint32_t)signal {
  [self.petView signal:signal];
}

- (void)beginGestureAt:(NSPoint)screenPoint {
  self.gestureActive = YES;
  self.gestureMoved = NO;
  _dragPersistence.begin();
  self.gestureMouseOrigin = screenPoint;
  self.gesturePanelOrigin = self.panel.frame.origin;
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
  [self.panel
      setFrameOrigin:NSMakePoint(self.gesturePanelOrigin.x + delta.x,
                                 self.gesturePanelOrigin.y + delta.y)];
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

  NSScreen *screen = [self screenForPanel];
  if (screen != nil) {
    NSRect frame = self.panel.frame;
    const NSRect safeFrame = screen.visibleFrame;
    const PetPanelFrame clamped = PetClampPanelToDisplay(
        {frame.origin.x, frame.origin.y, frame.size.width, frame.size.height},
        {safeFrame.origin.x, safeFrame.origin.y, safeFrame.size.width,
         safeFrame.size.height, screen.backingScaleFactor, 0},
        kPetSafeInset);
    frame.origin = NSMakePoint(clamped.x, clamped.y);
    [self.panel setFrameOrigin:frame.origin];
    [self syncCoreHitTargetFrame];
  }
  [self updateDisplaySelectionAndEmit:NO];
  [self refreshCaptureWithPermissionRequest:NO];
  if (shouldPersist && _callback != nullptr) {
    const NSPoint origin = self.panel.frame.origin;
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
  if (_callback != nullptr) {
    _callback(kPetCallbackDropExited, nullptr, 0.0, 0.0, self.displayID);
  }
}

- (void)dropCompleted {
  [self.petView completeDrop];
}

- (void)syncCoreHitTargetFrame {
  const NSRect visualFrame = self.panel.frame;
  const PetEffectGeometry geometry =
      PetEffectGeometryForSize(NSWidth(visualFrame));
  const CGFloat side = (CGFloat)(geometry.hit_radius * 2.0);
  const NSRect coreHitTargetFrame =
      NSMakeRect(NSMidX(visualFrame) - side / 2.0,
                 NSMidY(visualFrame) - side / 2.0, side, side);
  [self.coreHitTargetPanel setFrame:coreHitTargetFrame display:NO];
}

- (NSScreen *)screenForPanel {
  NSArray<NSScreen *> *screens = NSScreen.screens;
  if (screens.count == 0) {
    return nil;
  }
  const NSRect petFrame = self.panel.frame;
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
  const PetPanelFrame panel = {petFrame.origin.x, petFrame.origin.y,
                               petFrame.size.width, petFrame.size.height};
  const size_t selected =
      PetGreatestIntersectionDisplayIndex(panel, frames.data(), frames.size());
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
    return;
  }
  self.displayID = displayID;
  if (emit && _callback != nullptr) {
    const NSPoint origin = self.panel.frame.origin;
    _callback(kPetCallbackDisplayChanged, nullptr, origin.x, origin.y,
              displayID);
  }
}

- (void)screenParametersChanged:(NSNotification *)notification {
  (void)notification;
  if (_windowLifecycle.destroyed()) {
    return;
  }
  NSScreen *screen = [self screenForPanel];
  if (screen == nil) {
    mac_capture_stop(_captureHandle);
    return;
  }
  NSRect frame = self.panel.frame;
  const NSRect safeFrame = screen.visibleFrame;
  const PetPanelFrame clamped = PetClampPanelToDisplay(
      {frame.origin.x, frame.origin.y, frame.size.width, frame.size.height},
      {safeFrame.origin.x, safeFrame.origin.y, safeFrame.size.width,
       safeFrame.size.height, screen.backingScaleFactor, 0},
      kPetSafeInset);
  frame.origin = NSMakePoint(clamped.x, clamped.y);
  [self.panel setFrameOrigin:frame.origin];
  [self syncCoreHitTargetFrame];
  [self updateDisplaySelectionAndEmit:NO];
  _captureConfiguration.invalidate();
  [self refreshCaptureWithPermissionRequest:NO];
  if (_callback != nullptr) {
    _callback(kPetCallbackDisplayChanged, nullptr, frame.origin.x,
              frame.origin.y, self.displayID);
  }
}

- (void)workspaceWillSleep:(NSNotification *)notification {
  (void)notification;
  if (_windowLifecycle.destroyed() || _sleeping) {
    return;
  }
  _sleeping = YES;
  mac_capture_stop(_captureHandle);
  _captureConfiguration.invalidate();
  [self.petView setAnimating:NO];
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
    [self.petView setAnimating:YES];
  }
  if (_callback != nullptr) {
    _callback(kPetCallbackWake, nullptr, 0.0, 0.0, self.displayID);
  }
}

- (void)refreshCaptureWithPermissionRequest:(BOOL)requestPermission {
  if (!_hasConfig || _captureHandle == nullptr) {
    return;
  }
  const PetRendererDecision renderer =
      PetRendererDecisionForMetalAvailability(self.petView.metalAvailable);
  const BOOL visible = _windowLifecycle.visual_visible();
  const BOOL captureVisible = visible && !_sleeping &&
                              renderer.real_effect_available &&
                              !renderer.stop_capture;
  if (!captureVisible || _config.mode != 0) {
    const PetCaptureRegion emptyRegion = {};
    const PetCaptureConfigurationKey key = {
        _config.mode, captureVisible != NO, emptyRegion};
    if (_captureConfiguration.should_configure(key, requestPermission)) {
      self.captureRegion = emptyRegion;
      mac_capture_configure(_captureHandle, emptyRegion, _config.mode == 0,
                            captureVisible, requestPermission, _config.fps);
    }
    return;
  }
  NSScreen *screen = [self screenForPanel];
  if (screen == nil) {
    const PetCaptureRegion emptyRegion = {};
    const PetCaptureConfigurationKey key = {_config.mode, false, emptyRegion};
    if (_captureConfiguration.should_configure(key, requestPermission)) {
      self.captureRegion = emptyRegion;
      mac_capture_configure(_captureHandle, emptyRegion, true, false,
                            requestPermission, _config.fps);
    }
    return;
  }
  NSNumber *screenNumber = screen.deviceDescription[@"NSScreenNumber"];
  const NSRect panelFrame = self.panel.frame;
  const NSRect screenFrame = screen.frame;
  const PetPanelFrame panel = {
      panelFrame.origin.x,
      panelFrame.origin.y,
      panelFrame.size.width,
      panelFrame.size.height,
  };
  const PetScreenFrame display = {
      screenFrame.origin.x,
      screenFrame.origin.y,
      screenFrame.size.width,
      screenFrame.size.height,
      screen.backingScaleFactor,
      screenNumber.unsignedIntValue,
  };
  const PetCaptureRegion region = PetCaptureRegionForPanel(panel, display);
  const PetCaptureConfigurationKey key = {_config.mode, true, region};
  if (_captureConfiguration.should_configure(key, requestPermission)) {
    self.captureRegion = region;
    mac_capture_configure(_captureHandle, region, true, true,
                          requestPermission, _config.fps);
  }
}

- (void)rendererBecameUnavailable {
  mac_capture_stop(_captureHandle);
  if (_callback != nullptr) {
    _callback(kPetCallbackCaptureFailed, "metal_unavailable", 0.0, 0.0,
              self.displayID);
  }
}

- (IOSurfaceRef)copyLatestSurface {
  return mac_capture_copy_latest_surface(_captureHandle);
}

- (uint32_t)captureState {
  return mac_capture_state(_captureHandle);
}

- (uint32_t)rendererState {
  return PetRendererDecisionForMetalAvailability(self.petView.metalAvailable)
      .state;
}

- (uint32_t)shutdown {
  uint32_t shutdownState = PET_SHUTDOWN_COMPLETE;
  [self.petView setAnimating:NO];
  if (_captureHandle != nullptr) {
    shutdownState = mac_capture_destroy(_captureHandle);
    _captureHandle = nullptr;
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
  self.petView.petHost = nil;
  [self.panel removeChildWindow:self.coreHitTargetPanel];
  [self.coreHitTargetPanel close];
  self.coreHitTargetPanel.contentView = nil;
  [self.panel close];
  self.panel.contentView = nil;
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
