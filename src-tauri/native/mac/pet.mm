#import "bridge.h"
#import "pet_lifecycle.h"

#import <AppKit/AppKit.h>
#import <Metal/Metal.h>
#import <QuartzCore/QuartzCore.h>
#import <dispatch/dispatch.h>

#include <math.h>
#include <stddef.h>

static const uint32_t kPetAbiVersion = 1;
static const CGFloat kPetMinimumSize = 120.0;
static const CGFloat kPetMaximumSize = 360.0;

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

@class BPPetHost;

@interface BPPetPanel : NSPanel
@end

@interface BPPetView : NSView
- (void)setReduceMotion:(BOOL)reduceMotion;
- (void)setAnimating:(BOOL)animating;
- (void)pulse;
@end

@interface BPPetInteractionView : NSView
@property(nonatomic, weak) BPPetHost *petHost;
@end

@interface BPPetHost : NSObject
@property(nonatomic, strong) BPPetPanel *panel;
@property(nonatomic, strong) BPPetPanel *interactionPanel;
@property(nonatomic, strong) BPPetView *petView;
@property(nonatomic, strong) BPPetInteractionView *interactionView;
@property(nonatomic, assign) BOOL gestureActive;
@property(nonatomic, assign) NSPoint gestureMouseOrigin;
@property(nonatomic, assign) NSPoint gesturePanelOrigin;
- (instancetype)initWithCallback:(PetCallback)callback;
- (void)applyConfig:(PetConfig)config;
- (void)show;
- (void)hide;
- (void)reset;
- (void)signal:(uint32_t)signal;
- (void)beginGestureAt:(NSPoint)screenPoint;
- (void)continueGestureAt:(NSPoint)screenPoint;
- (void)endGesture;
- (void)syncInteractionFrame;
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
  const CGFloat ringWidth = MAX(3.0, CGRectGetWidth(bounds) * 0.035);
  const CGFloat inset = ringWidth * 1.6;
  const CGRect diskFrame = CGRectInset(bounds, inset, inset);

  [CATransaction begin];
  [CATransaction setDisableActions:YES];
  _diskLayer.frame = diskFrame;
  _diskLayer.cornerRadius = CGRectGetWidth(diskFrame) / 2.0;
  _ringLayer.frame = bounds;
  _ringMask.frame = bounds;
  _ringMask.lineWidth = ringWidth;
  CGPathRef ringPath =
      CGPathCreateWithEllipseInRect(CGRectInset(bounds, inset, inset), nullptr);
  _ringMask.path = ringPath;
  CGPathRelease(ringPath);
  [CATransaction commit];
}

- (void)setReduceMotion:(BOOL)reduceMotion {
  if (_reduceMotion == reduceMotion) {
    return;
  }
  _reduceMotion = reduceMotion;
  if (reduceMotion || !_animating) {
    [_diskLayer removeAllAnimations];
    [_ringLayer removeAllAnimations];
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
  }
}

- (void)installAnimations {
  if (_reduceMotion || !_animating) {
    return;
  }

  CABasicAnimation *pulse =
      [CABasicAnimation animationWithKeyPath:@"transform.scale"];
  pulse.fromValue = @0.96;
  pulse.toValue = @1.02;
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
}

- (void)pulse {
  if (!_animating) {
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

@end

@implementation BPPetInteractionView

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

@end

@implementation BPPetHost {
  PetCallback _callback;
  PetWindowLifecycle _windowLifecycle;
}

- (instancetype)initWithCallback:(PetCallback)callback {
  self = [super init];
  if (self) {
    _callback = callback;
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
    _panel.ignoresMouseEvents = YES;
    _panel.collectionBehavior =
        NSWindowCollectionBehaviorCanJoinAllSpaces |
        NSWindowCollectionBehaviorFullScreenAuxiliary;

    _petView = [[BPPetView alloc] initWithFrame:frame];
    _petView.autoresizingMask = NSViewWidthSizable | NSViewHeightSizable;
    _panel.contentView = _petView;

    const CGFloat interactionSide = (CGFloat)PetInteractionSide(220.0);
    const NSRect interactionFrame =
        NSMakeRect(0.0, 0.0, interactionSide, interactionSide);
    _interactionPanel = [[BPPetPanel alloc]
        initWithContentRect:interactionFrame
                  styleMask:(NSWindowStyleMaskBorderless |
                             NSWindowStyleMaskNonactivatingPanel)
                    backing:NSBackingStoreBuffered
                      defer:NO];
    _interactionPanel.opaque = NO;
    _interactionPanel.backgroundColor = NSColor.clearColor;
    _interactionPanel.hasShadow = NO;
    _interactionPanel.level = NSFloatingWindowLevel;
    _interactionPanel.hidesOnDeactivate = NO;
    _interactionPanel.releasedWhenClosed = NO;
    _interactionPanel.restorable = NO;
    _interactionPanel.ignoresMouseEvents = NO;
    _interactionPanel.collectionBehavior =
        NSWindowCollectionBehaviorCanJoinAllSpaces |
        NSWindowCollectionBehaviorFullScreenAuxiliary;

    _interactionView =
        [[BPPetInteractionView alloc] initWithFrame:interactionFrame];
    _interactionView.autoresizingMask =
        NSViewWidthSizable | NSViewHeightSizable;
    _interactionView.petHost = self;
    _interactionPanel.contentView = _interactionView;
    [_panel addChildWindow:_interactionPanel ordered:NSWindowAbove];
    [self reset];
  }
  return self;
}

- (void)applyConfig:(PetConfig)config {
  const NSRect oldFrame = self.panel.frame;
  const NSPoint center = NSMakePoint(NSMidX(oldFrame), NSMidY(oldFrame));
  const CGFloat size = (CGFloat)config.size;
  const NSRect frame =
      NSMakeRect(center.x - size / 2.0, center.y - size / 2.0, size, size);
  [self.panel setFrame:frame display:YES animate:NO];
  [self syncInteractionFrame];
  [self.petView setReduceMotion:config.reduce_motion != 0];

  if (config.visible != 0) {
    [self show];
  } else {
    [self hide];
  }
}

- (void)show {
  if (_windowLifecycle.destroyed()) {
    return;
  }
  _windowLifecycle.show();
  [self syncInteractionFrame];
  [self.petView setAnimating:YES];
  [self.panel orderFrontRegardless];
  [self.interactionPanel orderFrontRegardless];
}

- (void)hide {
  self.gestureActive = NO;
  _windowLifecycle.hide();
  [self.petView setAnimating:NO];
  [self.interactionPanel orderOut:nil];
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
  [self syncInteractionFrame];
}

- (void)signal:(uint32_t)signal {
  (void)signal;
  [self.petView pulse];
}

- (void)beginGestureAt:(NSPoint)screenPoint {
  self.gestureActive = YES;
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
  [self.panel
      setFrameOrigin:NSMakePoint(self.gesturePanelOrigin.x + delta.x,
                                 self.gesturePanelOrigin.y + delta.y)];
  [self syncInteractionFrame];
}

- (void)endGesture {
  self.gestureActive = NO;
}

- (void)syncInteractionFrame {
  const NSRect visualFrame = self.panel.frame;
  const CGFloat side =
      (CGFloat)PetInteractionSide(NSWidth(visualFrame));
  const NSRect interactionFrame =
      NSMakeRect(NSMidX(visualFrame) - side / 2.0,
                 NSMidY(visualFrame) - side / 2.0, side, side);
  [self.interactionPanel setFrame:interactionFrame display:NO];
}

- (void)shutdown {
  [self hide];
  _windowLifecycle.destroy();
  self.interactionView.petHost = nil;
  [self.panel removeChildWindow:self.interactionPanel];
  [self.interactionPanel close];
  self.interactionPanel.contentView = nil;
  [self.panel close];
  self.panel.contentView = nil;
  self.interactionView = nil;
  self.interactionPanel = nil;
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

static bool IsValidConfig(PetConfig config) {
  const bool validFps =
      config.fps == 0 || config.fps == 30 || config.fps == 60;
  return config.abi_version == kPetAbiVersion && config.mode <= 1 &&
         isfinite(config.size) && config.size >= kPetMinimumSize &&
         config.size <= kPetMaximumSize && validFps && config.visible <= 1 &&
         config.reduce_motion <= 1;
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
  RunOnMain(^{
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

extern "C" uint32_t pet_abi_version(void) {
  return kPetAbiVersion;
}
