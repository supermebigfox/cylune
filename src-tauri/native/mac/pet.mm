#import "bridge.h"
#import "pet_lifecycle.h"
#import "pet_visual_state.h"

#import <AppKit/AppKit.h>
#import <Metal/Metal.h>
#import <QuartzCore/QuartzCore.h>
#import <dispatch/dispatch.h>

#include <math.h>
#include <stddef.h>

static const uint32_t kPetAbiVersion = 1;
static const uint32_t kPetCallbackClicked = 1;
static const uint32_t kPetCallbackMoved = 2;
static const uint32_t kPetCallbackDropEntered = 3;
static const uint32_t kPetCallbackDropExited = 4;
static const uint32_t kPetCallbackFileDropped = 5;
static const uint32_t kPetCallbackDisplayChanged = 6;
static const uint32_t kPetCallbackSleep = 9;
static const uint32_t kPetCallbackWake = 10;
static const CGFloat kPetMinimumSize = 120.0;
static const CGFloat kPetMaximumSize = 360.0;
static const CGFloat kPetDragThreshold = 4.0;
static const CGFloat kPetSafeInset = 16.0;

static_assert(sizeof(PetConfig) == 32, "PetConfig ABI size changed");
static_assert(alignof(PetConfig) == 8, "PetConfig ABI alignment changed");
static_assert(offsetof(PetConfig, abi_version) == 0,
              "PetConfig abi_version offset changed");
static_assert(offsetof(PetConfig, mode) == 4,
              "PetConfig mode offset changed");
static_assert(offsetof(PetConfig, size) == 8,
              "PetConfig size offset changed");
static_assert(offsetof(PetConfig, fps) == 16,
              "PetConfig fps offset changed");
static_assert(offsetof(PetConfig, visible) == 20,
              "PetConfig visible offset changed");
static_assert(offsetof(PetConfig, pending_count) == 24,
              "PetConfig pending_count offset changed");
static_assert(offsetof(PetConfig, reduce_motion) == 28,
              "PetConfig reduce_motion offset changed");
static_assert(offsetof(PetConfig, request_permission) == 29,
              "PetConfig request_permission offset changed");

@class BPPetHost;

@interface BPPetPanel : NSPanel
@end

@interface BPPetView : NSView
- (void)setReduceMotion:(BOOL)reduceMotion;
- (void)setAnimating:(BOOL)animating;
- (void)setPendingCount:(uint32_t)pendingCount;
- (void)signal:(uint32_t)signal;
- (void)pulse;
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
@property(nonatomic, assign) BOOL observingScreenChanges;
@property(nonatomic, assign) BOOL observingWorkspace;
- (instancetype)initWithCallback:(PetCallback)callback;
- (void)applyConfig:(PetConfig)config;
- (void)show;
- (void)hide;
- (void)reset;
- (void)signal:(uint32_t)signal;
- (void)beginGestureAt:(NSPoint)screenPoint;
- (void)continueGestureAt:(NSPoint)screenPoint;
- (void)endGesture;
- (void)dragEntered;
- (void)dragExited;
- (void)syncCoreHitTargetFrame;
- (NSScreen *)screenForPanel;
- (void)updateDisplaySelectionAndEmit:(BOOL)emit;
- (void)screenParametersChanged:(NSNotification *)notification;
- (void)workspaceWillSleep:(NSNotification *)notification;
- (void)workspaceDidWake:(NSNotification *)notification;
- (void)refreshCaptureWithPermissionRequest:(BOOL)requestPermission;
- (uint32_t)captureState;
- (void)shutdown;
@end

@interface BPPetBridge : NSObject
@property(nonatomic, assign) PetCallback callback;
@property(nonatomic, strong) BPPetHost *host;
@property(nonatomic, assign) BOOL destroyed;
- (instancetype)initWithCallback:(PetCallback)callback;
- (void)ensureHost;
- (void)shutdown;
@end

@implementation BPPetPanel

- (BOOL)canBecomeKeyWindow {
  return NO;
}

- (BOOL)canBecomeMainWindow {
  return NO;
}

@end

@implementation BPPetView {
  CALayer *_diskLayer;
  CAGradientLayer *_ringLayer;
  CAShapeLayer *_ringMask;
  CALayer *_pendingDotsLayer;
  NSMutableArray<CALayer *> *_pendingDotLayers;
  CAShapeLayer *_signalLayer;
  PetVisualState _visualState;
  BOOL _reduceMotion;
  BOOL _animating;
}

- (instancetype)initWithFrame:(NSRect)frame {
  self = [super initWithFrame:frame];
  if (self) {
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
  }
  return self;
}

- (CALayer *)makeBackingLayer {
  id<MTLDevice> device = MTLCreateSystemDefaultDevice();
  if (device != nil) {
    CAMetalLayer *metalLayer = [CAMetalLayer layer];
    metalLayer.device = device;
    metalLayer.pixelFormat = MTLPixelFormatBGRA8Unorm;
    metalLayer.framebufferOnly = YES;
    return metalLayer;
  }
  return [CALayer layer];
}

- (BOOL)isFlipped {
  return YES;
}

- (void)layout {
  [super layout];
  const CGRect bounds = self.bounds;
  const CGFloat effectDiameter =
      MIN(CGRectGetWidth(bounds), CGRectGetHeight(bounds));
  const PetEventHorizonGeometry geometry =
      PetEventHorizonGeometryForEffectDiameter(effectDiameter);
  const CGFloat ringWidth = MAX(3.0, effectDiameter * 0.035);
  const CGRect effectFrame =
      CGRectMake(CGRectGetMidX(bounds) -
                     geometry.decorative_effect_diameter / 2.0,
                 CGRectGetMidY(bounds) -
                     geometry.decorative_effect_diameter / 2.0,
                 geometry.decorative_effect_diameter,
                 geometry.decorative_effect_diameter);
  const CGRect eventHorizonFrame =
      CGRectMake(CGRectGetMidX(bounds) -
                     geometry.event_horizon_diameter / 2.0,
                 CGRectGetMidY(bounds) -
                     geometry.event_horizon_diameter / 2.0,
                 geometry.event_horizon_diameter,
                 geometry.event_horizon_diameter);

  [CATransaction begin];
  [CATransaction setDisableActions:YES];
  _diskLayer.frame = eventHorizonFrame;
  _diskLayer.cornerRadius =
      geometry.event_horizon_diameter / 2.0;
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
      MIN(8.0, MAX(4.0, effectDiameter * 0.032));
  const uint32_t pendingCount = _visualState.pending_dot_count();
  for (uint32_t index = 0; index < pendingCount; ++index) {
    const PetPendingDotPlacement placement =
        PetPendingDotPlacementForIndex(index, pendingCount);
    const CGFloat orbitRadius =
        effectDiameter * 0.5 * placement.normalized_radius;
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
  if (_reduceMotion) {
    CABasicAnimation *fade =
        [CABasicAnimation animationWithKeyPath:@"opacity"];
    fade.fromValue = @0.45;
    fade.toValue = @1.0;
    fade.duration = 0.18;
    fade.autoreverses = YES;
    [_ringLayer addAnimation:fade forKey:@"pet.signal"];
    return;
  }
  CABasicAnimation *signal =
      [CABasicAnimation animationWithKeyPath:@"transform.scale"];
  signal.fromValue = @1.0;
  signal.toValue = @1.08;
  signal.duration = 0.12;
  signal.autoreverses = YES;
  [_ringLayer addAnimation:signal forKey:@"pet.signal"];
}

- (void)signal:(uint32_t)signal {
  if (!_animating) {
    return;
  }
  _visualState.apply_signal(signal);
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
  fade.duration = _reduceMotion ? 0.28 : 0.42;
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

@implementation BPCoreHitTargetView

- (instancetype)initWithFrame:(NSRect)frame {
  self = [super initWithFrame:frame];
  if (self) {
    [self registerForDraggedTypes:@[ NSPasteboardTypeFileURL ]];
  }
  return self;
}

- (void)mouseDown:(NSEvent *)event {
  (void)event;
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
  (void)sender;
  [self.petHost dragEntered];
  return NSDragOperationCopy;
}

- (void)draggingExited:(nullable id<NSDraggingInfo>)sender {
  (void)sender;
  [self.petHost dragExited];
}

- (BOOL)performDragOperation:(id<NSDraggingInfo>)sender {
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

@end

@implementation BPPetHost {
  PetCallback _callback;
  PetWindowLifecycle _windowLifecycle;
  void *_captureHandle;
  PetConfig _config;
  BOOL _hasConfig;
  BOOL _sleeping;
}

- (instancetype)initWithCallback:(PetCallback)callback {
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

    _petView = [[BPPetView alloc] initWithFrame:frame];
    _petView.autoresizingMask = NSViewWidthSizable | NSViewHeightSizable;
    _panel.contentView = _petView;

    const PetEventHorizonGeometry geometry =
        PetEventHorizonGeometryForEffectDiameter(220.0);
    const CGFloat coreHitTargetSide = (CGFloat)geometry.core_hit_target_side;
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
  NSRect frame =
      NSMakeRect(center.x - size / 2.0, center.y - size / 2.0, size, size);
  NSScreen *screen = [self screenForPanel];
  if (screen != nil) {
    const NSRect safeFrame = screen.visibleFrame;
    const CGFloat minimumX = NSMinX(safeFrame) + kPetSafeInset;
    const CGFloat maximumX = NSMaxX(safeFrame) - size - kPetSafeInset;
    const CGFloat minimumY = NSMinY(safeFrame) + kPetSafeInset;
    const CGFloat maximumY = NSMaxY(safeFrame) - size - kPetSafeInset;
    frame.origin.x =
        minimumX <= maximumX ? MIN(MAX(frame.origin.x, minimumX), maximumX)
                             : NSMidX(safeFrame) - size / 2.0;
    frame.origin.y =
        minimumY <= maximumY ? MIN(MAX(frame.origin.y, minimumY), maximumY)
                             : NSMidY(safeFrame) - size / 2.0;
  }
  [self.panel setFrame:frame display:YES animate:NO];
  [self syncCoreHitTargetFrame];
  [self updateDisplaySelectionAndEmit:NO];
  [self.petView setReduceMotion:config.reduce_motion != 0];
  [self.petView setPendingCount:config.pending_count];

  if (config.visible != 0) {
    [self show];
  } else {
    [self hide];
  }
  if (config.visible != 0 && config.request_permission != 0) {
    [self refreshCaptureWithPermissionRequest:config.request_permission != 0];
  }
  _config.request_permission = 0;
}

- (void)show {
  if (_windowLifecycle.destroyed()) {
    return;
  }
  _windowLifecycle.show();
  [self syncCoreHitTargetFrame];
  [self.petView setAnimating:YES];
  [self.panel orderFrontRegardless];
  [self.coreHitTargetPanel orderFrontRegardless];
  [self refreshCaptureWithPermissionRequest:NO];
}

- (void)hide {
  self.gestureActive = NO;
  _windowLifecycle.hide();
  [self.petView setAnimating:NO];
  mac_capture_stop(_captureHandle);
  [self.coreHitTargetPanel orderOut:nil];
  [self.panel orderOut:nil];
}

- (void)reset {
  NSScreen *screen = NSScreen.mainScreen ?: NSScreen.screens.firstObject;
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
  [self.panel
      setFrameOrigin:NSMakePoint(self.gesturePanelOrigin.x + delta.x,
                                 self.gesturePanelOrigin.y + delta.y)];
  [self syncCoreHitTargetFrame];
  [self updateDisplaySelectionAndEmit:YES];
}

- (void)endGesture {
  if (!self.gestureActive) {
    return;
  }
  const BOOL moved = self.gestureMoved;
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
    const CGFloat minimumX = NSMinX(safeFrame) + kPetSafeInset;
    const CGFloat maximumX =
        NSMaxX(safeFrame) - NSWidth(frame) - kPetSafeInset;
    const CGFloat minimumY = NSMinY(safeFrame) + kPetSafeInset;
    const CGFloat maximumY =
        NSMaxY(safeFrame) - NSHeight(frame) - kPetSafeInset;
    frame.origin.x =
        minimumX <= maximumX ? MIN(MAX(frame.origin.x, minimumX), maximumX)
                             : NSMidX(safeFrame) - NSWidth(frame) / 2.0;
    frame.origin.y =
        minimumY <= maximumY ? MIN(MAX(frame.origin.y, minimumY), maximumY)
                             : NSMidY(safeFrame) - NSHeight(frame) / 2.0;
    [self.panel setFrameOrigin:frame.origin];
    [self syncCoreHitTargetFrame];
  }
  [self updateDisplaySelectionAndEmit:YES];
  [self refreshCaptureWithPermissionRequest:NO];
  if (_callback != nullptr) {
    const NSPoint origin = self.panel.frame.origin;
    _callback(kPetCallbackMoved, nullptr, origin.x, origin.y, self.displayID);
  }
}

- (void)dragEntered {
  [self.petView pulse];
  if (_callback != nullptr) {
    _callback(kPetCallbackDropEntered, nullptr, 0.0, 0.0, self.displayID);
  }
}

- (void)dragExited {
  if (_callback != nullptr) {
    _callback(kPetCallbackDropExited, nullptr, 0.0, 0.0, self.displayID);
  }
}

- (void)syncCoreHitTargetFrame {
  const NSRect visualFrame = self.panel.frame;
  const PetEventHorizonGeometry geometry =
      PetEventHorizonGeometryForEffectDiameter(NSWidth(visualFrame));
  const CGFloat side = (CGFloat)geometry.core_hit_target_side;
  const NSRect coreHitTargetFrame =
      NSMakeRect(NSMidX(visualFrame) - side / 2.0,
                 NSMidY(visualFrame) - side / 2.0, side, side);
  [self.coreHitTargetPanel setFrame:coreHitTargetFrame display:NO];
}

- (NSScreen *)screenForPanel {
  NSArray<NSScreen *> *screens = NSScreen.screens;
  NSScreen *selected = screens.firstObject;
  CGFloat greatestArea = -1.0;
  const NSRect petFrame = self.panel.frame;
  for (NSScreen *screen in screens) {
    const NSRect intersection = NSIntersectionRect(petFrame, screen.frame);
    const CGFloat area =
        NSWidth(intersection) * NSHeight(intersection);
    if (area > greatestArea) {
      greatestArea = area;
      selected = screen;
    }
  }
  return selected;
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
  const CGFloat minimumX = NSMinX(safeFrame) + kPetSafeInset;
  const CGFloat maximumX =
      NSMaxX(safeFrame) - NSWidth(frame) - kPetSafeInset;
  const CGFloat minimumY = NSMinY(safeFrame) + kPetSafeInset;
  const CGFloat maximumY =
      NSMaxY(safeFrame) - NSHeight(frame) - kPetSafeInset;
  frame.origin.x =
      minimumX <= maximumX ? MIN(MAX(frame.origin.x, minimumX), maximumX)
                           : NSMidX(safeFrame) - NSWidth(frame) / 2.0;
  frame.origin.y =
      minimumY <= maximumY ? MIN(MAX(frame.origin.y, minimumY), maximumY)
                           : NSMidY(safeFrame) - NSHeight(frame) / 2.0;
  [self.panel setFrameOrigin:frame.origin];
  [self syncCoreHitTargetFrame];
  [self updateDisplaySelectionAndEmit:NO];
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
  const BOOL visible = _windowLifecycle.visual_visible();
  if (_sleeping || !visible || _config.mode != 0) {
    mac_capture_configure(_captureHandle, {}, _config.mode == 0,
                          visible && !_sleeping, requestPermission,
                          _config.fps);
    return;
  }
  NSScreen *screen = [self screenForPanel];
  if (screen == nil) {
    mac_capture_configure(_captureHandle, {}, true, false, requestPermission,
                          _config.fps);
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
  mac_capture_configure(_captureHandle, region, true, true, requestPermission,
                        _config.fps);
}

- (uint32_t)captureState {
  return mac_capture_state(_captureHandle);
}

- (void)shutdown {
  [self hide];
  _windowLifecycle.destroy();
  if (self.observingScreenChanges) {
    [NSNotificationCenter.defaultCenter removeObserver:self];
    self.observingScreenChanges = NO;
  }
  if (self.observingWorkspace) {
    [NSWorkspace.sharedWorkspace.notificationCenter removeObserver:self];
    self.observingWorkspace = NO;
  }
  mac_capture_destroy(_captureHandle);
  _captureHandle = nullptr;
  [self.coreHitTargetView unregisterDraggedTypes];
  self.coreHitTargetView.petHost = nil;
  self.coreHitTargetView.callback = nullptr;
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
}

@end

@implementation BPPetBridge

- (instancetype)initWithCallback:(PetCallback)callback {
  self = [super init];
  if (self) {
    _callback = callback;
  }
  return self;
}

- (void)ensureHost {
  if (!self.destroyed && self.host == nil) {
    self.host = [[BPPetHost alloc] initWithCallback:self.callback];
  }
}

- (void)shutdown {
  if (self.destroyed) {
    return;
  }
  self.destroyed = YES;
  [self.host shutdown];
  self.host = nil;
  self.callback = nullptr;
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

static bool IsValidConfig(PetConfig config) {
  const bool validFps =
      config.fps == 0 || config.fps == 30 || config.fps == 60;
  return config.abi_version == kPetAbiVersion && config.mode <= 1 &&
         isfinite(config.size) && config.size >= kPetMinimumSize &&
         config.size <= kPetMaximumSize && validFps && config.visible <= 1 &&
         config.reduce_motion <= 1 && config.request_permission <= 1;
}

extern "C" void *pet_create(PetCallback callback,
                             const char *metal_source) {
  (void)metal_source;
  BPPetBridge *bridge = [[BPPetBridge alloc] initWithCallback:callback];
  void *handle = (__bridge_retained void *)bridge;
  RunOnMain(^{
    [bridge ensureHost];
  });
  return handle;
}

extern "C" void pet_destroy(void *handle) {
  if (handle == nullptr) {
    return;
  }
  BPPetBridge *bridge = (__bridge_transfer BPPetBridge *)handle;
  RunOnMain(^{
    [bridge shutdown];
  });
}

extern "C" bool pet_apply(void *handle, PetConfig config) {
  if (handle == nullptr || !IsValidConfig(config)) {
    return false;
  }
  BPPetBridge *bridge = (__bridge BPPetBridge *)handle;
  RunOnMainAndWait(^{
    if (!bridge.destroyed) {
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

extern "C" uint32_t pet_abi_version(void) {
  return kPetAbiVersion;
}
