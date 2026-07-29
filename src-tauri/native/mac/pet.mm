#import <AppKit/AppKit.h>
#import <CoreGraphics/CoreGraphics.h>

#import "bridge.h"
#import "pet_drop_state.h"
#import "pet_ingest_animation.h"
#import "pet_lifecycle.h"
#import "pet_position.h"
#import "tiyda/BlackHoleDesktop.h"
#import "tiyda/black_hole_params.h"

#include <cmath>
#include <vector>

static const uint32_t kPetAbiVersion = 1;
static const uint32_t kPetCallbackClicked = 1;
static const uint32_t kPetCallbackMoved = 2;
static const uint32_t kPetCallbackDropEntered = 3;
static const uint32_t kPetCallbackDropExited = 4;
static const uint32_t kPetCallbackFileDropped = 5;
static const uint32_t kPetCallbackDisplayChanged = 6;
static const uint32_t kPetCallbackPermissionChanged = 7;
static const uint32_t kPetCallbackSleep = 9;
static const uint32_t kPetCallbackWake = 10;
static const CGFloat kPetMinimumSize = 120.0;
static const CGFloat kPetMaximumSize = 900.0;
static const CGFloat kPetDragThreshold = 4.0;

@class BHPetHost;

@interface BHPetPanel : NSPanel
@end

@implementation BHPetPanel
- (BOOL)canBecomeKeyWindow { return NO; }
- (BOOL)canBecomeMainWindow { return NO; }
@end

@interface BHPetPane : NSObject
@property(nonatomic, strong) BHPetPanel *panel;
@property(nonatomic, strong) MetalBlackHoleView *blackHoleView;
@property(nonatomic, strong) NSScreen *screen;
@property(nonatomic) uint64_t displayID;
@end

@implementation BHPetPane
@end

@interface BHPetTargetView : NSView <NSDraggingDestination>
@property(nonatomic, weak) BHPetHost *host;
@property(nonatomic) BOOL fileHovering;
- (void)beginIngestingURL:(NSURL *)url
                  atPoint:(NSPoint)point
              ejectAfter:(BOOL)ejectAfter;
- (void)finishIngestAnimation;
@end

@interface BHPetHost : NSObject
- (instancetype)initWithCallback:(PetCallback)callback
                      metalSource:(NSString *)metalSource;
- (void)applyConfig:(PetConfig)config;
- (void)show;
- (void)hide;
- (void)reset;
- (void)signal:(uint32_t)signal;
- (void)finishDrop:(uint64_t)generation result:(uint32_t)result;
- (uint32_t)captureState;
- (uint32_t)rendererState;
- (uint32_t)shutdown;
- (void)beginManualDragAt:(NSPoint)screenPoint;
- (void)continueManualDragAt:(NSPoint)screenPoint;
- (void)endManualDrag;
- (NSURL *)supportedURLFromDraggingInfo:(id<NSDraggingInfo>)sender
                              fileKind:(uint32_t *)fileKind;
- (NSDragOperation)dragEnteredWithURL:(NSURL *)url
                            fileKind:(uint32_t)fileKind;
- (BOOL)performDropWithURL:(NSURL *)url fileKind:(uint32_t)fileKind;
- (void)dragExited;
- (void)setIngestProgress:(CGFloat)ingestProgress
            ejectProgress:(CGFloat)ejectProgress;
- (void)setDropHovering:(BOOL)hovering;
- (void)tickIngestAnimation;
@end

static NSWindowCollectionBehavior PetWindowBehavior(void) {
  return NSWindowCollectionBehaviorCanJoinAllSpaces |
         NSWindowCollectionBehaviorStationary |
         NSWindowCollectionBehaviorIgnoresCycle |
         NSWindowCollectionBehaviorFullScreenAuxiliary;
}

static std::vector<PetScreenFrame> PetScreenFrames(void) {
  std::vector<PetScreenFrame> frames;
  frames.reserve(NSScreen.screens.count);
  for (NSScreen *screen in NSScreen.screens) {
    const NSRect frame = screen.frame;
    NSNumber *number = screen.deviceDescription[@"NSScreenNumber"];
    frames.push_back({frame.origin.x, frame.origin.y, frame.size.width,
                      frame.size.height, screen.backingScaleFactor,
                      number.unsignedIntValue});
  }
  return frames;
}

static uint64_t PetDisplayID(NSScreen *screen) {
  return [screen.deviceDescription[@"NSScreenNumber"] unsignedLongLongValue];
}

static uint32_t PetFileKindForPath(NSString *path) {
  NSString *lower = path.lowercaseString;
  if ([lower hasSuffix:@".gcode.3mf"] || [lower hasSuffix:@".3mf"]) {
    return PET_FILE_3MF;
  }
  if ([lower hasSuffix:@".gcode"]) {
    return PET_FILE_GCODE;
  }
  return PET_FILE_OTHER;
}

static BOOL PetURLIsRegularFile(NSURL *url, uint32_t *fileKind) {
  if (url == nil || !url.fileURL) return NO;
  NSNumber *regular = nil;
  NSNumber *symbolicLink = nil;
  NSError *error = nil;
  BOOL read = [url getResourceValue:&regular
                             forKey:NSURLIsRegularFileKey
                              error:&error];
  if (!read || error != nil || !regular.boolValue) return NO;
  [url getResourceValue:&symbolicLink
                 forKey:NSURLIsSymbolicLinkKey
                  error:nil];
  if (symbolicLink.boolValue) return NO;
  if (fileKind != nullptr) *fileKind = PetFileKindForPath(url.path);
  return YES;
}

@implementation BHPetTargetView {
  BOOL _manualDragging;
  NSPoint _mouseDownScreenPoint;
  NSTimer *_hoverTimer;
  BOOL _ingestAnimationActive;
  BOOL _ejectAfterIngest;
  CFAbsoluteTime _ingestStartedAt;
  NSImage *_ingestFileIcon;
  NSPoint _ingestStartPoint;
  CGFloat _ingestStartRadius;
  CGFloat _ingestStartAngle;
}

- (instancetype)initWithFrame:(NSRect)frameRect {
  self = [super initWithFrame:frameRect];
  if (self) {
    self.wantsLayer = YES;
    self.layer.opaque = NO;
    [self registerForDraggedTypes:@[NSPasteboardTypeFileURL]];
    __weak BHPetTargetView *weakSelf = self;
    _hoverTimer =
        [NSTimer timerWithTimeInterval:1.0 / 60.0
                               repeats:YES
                                 block:^(__unused NSTimer *timer) {
                                   BHPetTargetView *strongSelf = weakSelf;
                                   if (!strongSelf) return;
                                   if (strongSelf->_ingestAnimationActive) {
                                     const CFTimeInterval elapsed =
                                         CFAbsoluteTimeGetCurrent() -
                                         strongSelf->_ingestStartedAt;
                                     const CFTimeInterval duration =
                                         kPetSwallowDurationSeconds +
                                         (strongSelf->_ejectAfterIngest
                                              ? kPetEjectDurationSeconds
                                              : 0.0);
                                     if (elapsed >= duration) {
                                       strongSelf->_ingestAnimationActive = NO;
                                       strongSelf->_ingestFileIcon = nil;
                                     }
                                   }
                                   if (strongSelf.fileHovering ||
                                       strongSelf->_ingestAnimationActive) {
                                     strongSelf.needsDisplay = YES;
                                   }
                                 }];
    [NSRunLoop.mainRunLoop addTimer:_hoverTimer
                           forMode:NSRunLoopCommonModes];
  }
  return self;
}

- (void)dealloc {
  [_hoverTimer invalidate];
}

- (BOOL)isOpaque { return NO; }

- (BOOL)pointInsideTarget:(NSPoint)point {
  const CGFloat side = MIN(NSWidth(self.bounds), NSHeight(self.bounds));
  return PetPointInsideDropTarget(point.x, point.y, side);
}

- (NSPoint)localDraggingPoint:(id<NSDraggingInfo>)sender {
  return [self convertPoint:sender.draggingLocation fromView:nil];
}

- (void)resetCursorRects {
  [self addCursorRect:self.bounds
               cursor:_manualDragging ? NSCursor.closedHandCursor
                                      : NSCursor.openHandCursor];
}

- (void)mouseDown:(NSEvent *)event {
  const NSPoint localPoint =
      [self convertPoint:event.locationInWindow fromView:nil];
  if (![self pointInsideTarget:localPoint]) return;
  _manualDragging = YES;
  _mouseDownScreenPoint = NSEvent.mouseLocation;
  [self.host beginManualDragAt:_mouseDownScreenPoint];
  [self.window invalidateCursorRectsForView:self];
}

- (void)mouseDragged:(NSEvent *)event {
  (void)event;
  if (_manualDragging) {
    [self.host continueManualDragAt:NSEvent.mouseLocation];
  }
}

- (void)mouseUp:(NSEvent *)event {
  (void)event;
  if (!_manualDragging) return;
  _manualDragging = NO;
  [self.host endManualDrag];
  [self.window invalidateCursorRectsForView:self];
}

- (NSURL *)supportedURL:(id<NSDraggingInfo>)sender
               fileKind:(uint32_t *)fileKind {
  return [self.host supportedURLFromDraggingInfo:sender fileKind:fileKind];
}

- (NSDragOperation)draggingEntered:(id<NSDraggingInfo>)sender {
  if (![self pointInsideTarget:[self localDraggingPoint:sender]]) {
    return NSDragOperationNone;
  }
  uint32_t fileKind = PET_FILE_NONE;
  NSURL *url = [self supportedURL:sender fileKind:&fileKind];
  NSDragOperation operation =
      [self.host dragEnteredWithURL:url fileKind:fileKind];
  self.fileHovering = operation != NSDragOperationNone;
  self.needsDisplay = YES;
  return operation;
}

- (NSDragOperation)draggingUpdated:(id<NSDraggingInfo>)sender {
  uint32_t fileKind = PET_FILE_NONE;
  NSURL *url = [self supportedURL:sender fileKind:&fileKind];
  const BOOL inside =
      [self pointInsideTarget:[self localDraggingPoint:sender]];
  if (url == nil || !inside) {
    if (self.fileHovering) {
      self.fileHovering = NO;
      self.needsDisplay = YES;
      [self.host dragExited];
    }
    return NSDragOperationNone;
  }
  if (!self.fileHovering) {
    const NSDragOperation operation =
        [self.host dragEnteredWithURL:url fileKind:fileKind];
    self.fileHovering = operation != NSDragOperationNone;
    self.needsDisplay = YES;
    return operation;
  }
  return NSDragOperationCopy;
}

- (void)draggingExited:(id<NSDraggingInfo>)sender {
  (void)sender;
  self.fileHovering = NO;
  self.needsDisplay = YES;
  [self.host dragExited];
}

- (void)draggingEnded:(id<NSDraggingInfo>)sender {
  (void)sender;
  if (self.fileHovering) {
    self.fileHovering = NO;
    self.needsDisplay = YES;
    [self.host dragExited];
  }
}

- (BOOL)prepareForDragOperation:(id<NSDraggingInfo>)sender {
  uint32_t fileKind = PET_FILE_NONE;
  return [self pointInsideTarget:[self localDraggingPoint:sender]] &&
         [self supportedURL:sender fileKind:&fileKind] != nil;
}

- (BOOL)performDragOperation:(id<NSDraggingInfo>)sender {
  const NSPoint dropPoint = [self localDraggingPoint:sender];
  if (![self pointInsideTarget:dropPoint]) return NO;
  uint32_t fileKind = PET_FILE_NONE;
  NSURL *url = [self supportedURL:sender fileKind:&fileKind];
  const BOOL accepted =
      [self.host performDropWithURL:url fileKind:fileKind];
  if (accepted) {
    [self beginIngestingURL:url
                    atPoint:dropPoint
                ejectAfter:fileKind == PET_FILE_OTHER];
  }
  self.fileHovering = NO;
  self.needsDisplay = YES;
  return accepted;
}

- (void)beginIngestingURL:(NSURL *)url
                  atPoint:(NSPoint)point
              ejectAfter:(BOOL)ejectAfter {
  _ingestAnimationActive = YES;
  _ejectAfterIngest = ejectAfter;
  _ingestStartedAt = CFAbsoluteTimeGetCurrent();
  _ingestFileIcon = [NSWorkspace.sharedWorkspace
      iconForFile:url.path].copy;
  _ingestStartPoint = point;
  const NSPoint center =
      NSMakePoint(NSMidX(self.bounds), NSMidY(self.bounds));
  const CGFloat dx = point.x - center.x;
  const CGFloat dy = point.y - center.y;
  const CGFloat radius = MIN(NSWidth(self.bounds), NSHeight(self.bounds)) * .48;
  _ingestStartRadius = MAX(hypot(dx, dy), radius * .72);
  _ingestStartAngle = hypot(dx, dy) > 1.0 ? atan2(dy, dx) : 0.0;
  self.needsDisplay = YES;
}

- (void)finishIngestAnimation {
  _ingestAnimationActive = NO;
  _ingestFileIcon = nil;
  self.needsDisplay = YES;
}

- (void)drawRect:(NSRect)dirtyRect {
  (void)dirtyRect;
  if (!PetShouldDrawDropOverlay(self.fileHovering,
                                _ingestAnimationActive)) {
    return;
  }
  CGFloat radius = MIN(NSWidth(self.bounds), NSHeight(self.bounds)) * 0.48;
  CGPoint center = CGPointMake(NSMidX(self.bounds), NSMidY(self.bounds));
  CFAbsoluteTime time = CFAbsoluteTimeGetCurrent();

  if (_ingestAnimationActive && _ingestFileIcon != nil) {
    const CFTimeInterval elapsed = time - _ingestStartedAt;
    CGFloat progress = 0;
    CGFloat orbitRadius = 0;
    CGFloat angle = _ingestStartAngle;
    CGFloat iconScale = 1;
    CGFloat alpha = 1;
    if (elapsed <= kPetSwallowDurationSeconds) {
      const CGFloat raw = PetSwallowProgress(elapsed);
      const CGFloat eased = PetEase(raw);
      progress = raw;
      orbitRadius = _ingestStartRadius * PetOrbitScale(raw);
      angle -= eased * M_PI * 4.5;
      iconScale = MAX(.04, 1.0 - eased);
    } else {
      const CGFloat raw = PetEjectProgress(elapsed);
      const CGFloat eased = PetEase(raw);
      progress = raw;
      orbitRadius = radius * .92 * eased;
      angle = _ingestStartAngle - M_PI * 4.5 - eased * M_PI * 1.35;
      iconScale = .08 + .92 * eased;
      alpha = raw < .82 ? 1.0 : MAX(0.0, (1.0 - raw) / .18);
    }
    const NSPoint iconCenter =
        NSMakePoint(center.x + cos(angle) * orbitRadius,
                    center.y + sin(angle) * orbitRadius);
    const CGFloat iconSide = MIN(72.0, radius * .52) * iconScale;
    [NSGraphicsContext saveGraphicsState];
    NSAffineTransform *transform = [NSAffineTransform transform];
    [transform translateXBy:iconCenter.x yBy:iconCenter.y];
    [transform rotateByRadians:-progress * M_PI * 5.0];
    [transform concat];
    [_ingestFileIcon
        drawInRect:NSMakeRect(-iconSide / 2.0, -iconSide / 2.0, iconSide,
                              iconSide)
          fromRect:NSZeroRect
         operation:NSCompositingOperationSourceOver
          fraction:alpha
    respectFlipped:YES
             hints:nil];
    [NSGraphicsContext restoreGraphicsState];
  }
}

@end

@implementation BHPetHost {
  PetCallback _callback;
  NSString *_metalSource;
  NSMutableArray<BHPetPane *> *_panes;
  BHPetPanel *_targetPanel;
  BHPetTargetView *_targetView;
  PetConfig _config;
  BOOL _hasConfig;
  PetWindowLifecycle _lifecycle;
  PetDropSession _dropSession;
  NSPoint _centerScreenPoint;
  CGFloat _visualSize;
  uint64_t _displayID;
  BOOL _gestureActive;
  BOOL _gestureMoved;
  NSPoint _gestureMouseOrigin;
  NSPoint _gestureCenterOrigin;
  NSTimer *_ingestTimer;
  CFAbsoluteTime _ingestStartedAt;
  CGFloat _ingestProgress;
  CGFloat _ejectProgress;
  BOOL _dropHovering;
  BOOL _pendingDropSupported;
  uint64_t _pendingDropGeneration;
  NSString *_pendingDropPath;
}

- (instancetype)initWithCallback:(PetCallback)callback
                      metalSource:(NSString *)metalSource {
  self = [super init];
  if (self) {
    _callback = callback;
    _metalSource = [metalSource copy];
    _panes = [NSMutableArray array];
    _visualSize = 300.0;
    const std::vector<PetScreenFrame> frames = PetScreenFrames();
    const PetScreenPoint center =
        PetPrimaryDisplayCenter(frames.data(), frames.size());
    _centerScreenPoint = NSMakePoint(center.x, center.y);

    _targetPanel = [[BHPetPanel alloc]
        initWithContentRect:NSMakeRect(0, 0, 108, 108)
                  styleMask:(NSWindowStyleMaskBorderless |
                             NSWindowStyleMaskNonactivatingPanel)
                    backing:NSBackingStoreBuffered
                      defer:NO];
    _targetPanel.opaque = NO;
    _targetPanel.backgroundColor = NSColor.clearColor;
    _targetPanel.hasShadow = NO;
    _targetPanel.level = NSFloatingWindowLevel + 1;
    _targetPanel.hidesOnDeactivate = NO;
    _targetPanel.releasedWhenClosed = NO;
    _targetPanel.restorable = NO;
    _targetPanel.ignoresMouseEvents = NO;
    _targetPanel.collectionBehavior = PetWindowBehavior();
    _targetView = [[BHPetTargetView alloc]
        initWithFrame:NSMakeRect(0, 0, 108, 108)];
    _targetView.autoresizingMask = NSViewWidthSizable | NSViewHeightSizable;
    _targetView.host = self;
    _targetPanel.contentView = _targetView;

    [self rebuildPanes];
    [self syncTargetFrame];
    [NSNotificationCenter.defaultCenter
        addObserver:self
           selector:@selector(screenParametersChanged:)
               name:NSApplicationDidChangeScreenParametersNotification
             object:nil];
    [NSWorkspace.sharedWorkspace.notificationCenter
        addObserver:self
           selector:@selector(workspaceWillSleep:)
               name:NSWorkspaceWillSleepNotification
             object:nil];
    [NSWorkspace.sharedWorkspace.notificationCenter
        addObserver:self
           selector:@selector(workspaceDidWake:)
               name:NSWorkspaceDidWakeNotification
             object:nil];
  }
  return self;
}

- (void)dealloc {
  [_ingestTimer invalidate];
  [NSNotificationCenter.defaultCenter removeObserver:self];
  [NSWorkspace.sharedWorkspace.notificationCenter removeObserver:self];
}

- (void)rebuildPanes {
  const BOOL shouldShow = _lifecycle.visible() && !_lifecycle.sleeping();
  for (BHPetPane *pane in _panes) {
    [pane.blackHoleView setCaptureEnabled:NO];
    [pane.blackHoleView setRenderingPaused:YES];
    [pane.panel orderOut:nil];
    [pane.panel close];
  }
  [_panes removeAllObjects];

  for (NSScreen *screen in NSScreen.screens) {
    BHPetPane *pane = [BHPetPane new];
    pane.screen = screen;
    pane.displayID = PetDisplayID(screen);
    pane.panel = [[BHPetPanel alloc]
        initWithContentRect:screen.frame
                  styleMask:(NSWindowStyleMaskBorderless |
                             NSWindowStyleMaskNonactivatingPanel)
                    backing:NSBackingStoreBuffered
                      defer:NO
                      screen:screen];
    pane.panel.opaque = NO;
    pane.panel.backgroundColor = NSColor.clearColor;
    pane.panel.hasShadow = NO;
    pane.panel.level = NSFloatingWindowLevel;
    pane.panel.hidesOnDeactivate = NO;
    pane.panel.releasedWhenClosed = NO;
    pane.panel.restorable = NO;
    pane.panel.ignoresMouseEvents = YES;
    pane.panel.collectionBehavior = PetWindowBehavior();

    pane.blackHoleView = [[MetalBlackHoleView alloc]
        initWithFrame:NSMakeRect(0, 0, NSWidth(screen.frame),
                                 NSHeight(screen.frame))
          metalSource:_metalSource];
    pane.blackHoleView.autoresizingMask =
        NSViewWidthSizable | NSViewHeightSizable;
    pane.panel.contentView = pane.blackHoleView;
    [_panes addObject:pane];
    if (shouldShow) [pane.panel orderFrontRegardless];
  }
  [self updatePanes];
}

- (NSInteger)framesPerSecondForScreen:(NSScreen *)screen {
  uint32_t refreshRate = 60;
  if (@available(macOS 12.0, *)) {
    refreshRate = (uint32_t)MAX(screen.maximumFramesPerSecond, 30);
  }
  const BHResolvedSettings settings = BHResolveSettings(
      {0.5f, 0.5f, (float)_visualSize, _hasConfig ? _config.fps : 0,
       _hasConfig ? static_cast<uint32_t>(_config.visual_style) : 0u},
      refreshRate);
  return settings.framesPerSecond;
}

- (void)updatePanes {
  const BOOL activeRendering =
      _lifecycle.visible() && !_lifecycle.sleeping();
  const BOOL captureEnabled =
      activeRendering && _hasConfig && _config.effective_mode == 0;
  const BHHoverEffect hoverEffect =
      BHResolveHoverEffect(_dropHovering ? 1.0f : 0.0f);
  for (BHPetPane *pane in _panes) {
    const BOOL active = pane.displayID == _displayID;
    MetalBlackHoleView *view = pane.blackHoleView;
    view.hidden = !active;
    view.blackHoleCenterInScreen = _centerScreenPoint;
    view.blackHoleSize = BHHoverVisualDiameter(_visualSize, hoverEffect);
    view.blackHoleBrightness = 1.0f;
    view.blackHoleSpeed = hoverEffect.rotationRate;
    view.blackHolePullGain = hoverEffect.pullGain;
    view.blackHoleIngestProgress = _ingestProgress;
    view.blackHoleEjectProgress = _ejectProgress;
    view.blackHoleStyle =
        _hasConfig && _config.visual_style == 0 ? BHStyleGargantua
                                                : BHStyleDefault;
    [view setTargetFramesPerSecond:[self framesPerSecondForScreen:pane.screen]];
    [view setCaptureEnabled:captureEnabled && active];
    [view setRenderingPaused:!activeRendering || !active];
  }
}

- (void)selectDisplay {
  if (_panes.count == 0) {
    _displayID = 0;
    return;
  }
  const std::vector<PetScreenFrame> frames = PetScreenFrames();
  size_t current = 0;
  for (size_t index = 0; index < frames.size(); ++index) {
    if (frames[index].display_id == _displayID) current = index;
  }
  const size_t selected = PetDisplayIndexForPoint(
      {_centerScreenPoint.x, _centerScreenPoint.y}, frames.data(),
      frames.size(), current);
  _displayID = frames[selected].display_id;
}

- (void)syncTargetFrame {
  const CGFloat side = PetDropTargetSide(_visualSize);
  [_targetPanel
      setFrame:NSMakeRect(_centerScreenPoint.x - side / 2.0,
                          _centerScreenPoint.y - side / 2.0, side, side)
       display:NO];
}

- (void)applyConfig:(PetConfig)config {
  _config = config;
  _hasConfig = YES;
  _visualSize =
      MIN(MAX((CGFloat)config.size, kPetMinimumSize), kPetMaximumSize);
  if (config.has_position && !_gestureActive) {
    const std::vector<PetScreenFrame> frames = PetScreenFrames();
    const PetScreenPoint recovered = PetRecoverCenter(
        {config.x + _visualSize / 2.0, config.y + _visualSize / 2.0},
        frames.data(), frames.size());
    _centerScreenPoint = NSMakePoint(recovered.x, recovered.y);
  }
  [self selectDisplay];
  [self syncTargetFrame];
  [self updatePanes];
  if (config.request_permission) {
    [self requestCapturePermission];
  }
  config.visible ? [self show] : [self hide];
  _config.request_permission = 0;
}

- (void)show {
  if (_lifecycle.destroyed()) return;
  _lifecycle.show();
  for (BHPetPane *pane in _panes) {
    [pane.panel orderFrontRegardless];
  }
  [_targetPanel orderFrontRegardless];
  [self updatePanes];
}

- (void)hide {
  _gestureActive = NO;
  _lifecycle.hide();
  _targetView.fileHovering = NO;
  [_targetPanel orderOut:nil];
  for (BHPetPane *pane in _panes) {
    [pane.blackHoleView setCaptureEnabled:NO];
    [pane.blackHoleView setRenderingPaused:YES];
    [pane.panel orderOut:nil];
  }
}

- (void)reset {
  const std::vector<PetScreenFrame> frames = PetScreenFrames();
  if (frames.empty()) return;
  const PetScreenPoint center =
      PetPrimaryDisplayCenter(frames.data(), frames.size());
  _centerScreenPoint = NSMakePoint(center.x, center.y);
  [self selectDisplay];
  [self syncTargetFrame];
  [self updatePanes];
}

- (void)signal:(uint32_t)signal {
  (void)signal;
}

- (void)beginManualDragAt:(NSPoint)screenPoint {
  _gestureActive = YES;
  _gestureMoved = NO;
  _gestureMouseOrigin = screenPoint;
  _gestureCenterOrigin = _centerScreenPoint;
}

- (void)continueManualDragAt:(NSPoint)screenPoint {
  if (!_gestureActive) return;
  const double dx = screenPoint.x - _gestureMouseOrigin.x;
  const double dy = screenPoint.y - _gestureMouseOrigin.y;
  if (!_gestureMoved && hypot(dx, dy) < kPetDragThreshold) return;
  _gestureMoved = YES;
  const std::vector<PetScreenFrame> frames = PetScreenFrames();
  const PetScreenPoint clamped = PetClampPointToDisplays(
      {_gestureCenterOrigin.x + dx, _gestureCenterOrigin.y + dy},
      frames.data(), frames.size());
  _centerScreenPoint = NSMakePoint(clamped.x, clamped.y);
  [self selectDisplay];
  [self syncTargetFrame];
  [self updatePanes];
}

- (void)endManualDrag {
  if (!_gestureActive) return;
  _gestureActive = NO;
  if (!_gestureMoved) {
    if (_callback != nullptr) {
      _callback(kPetCallbackClicked, nullptr, 0, 0, _displayID);
    }
    return;
  }
  _gestureMoved = NO;
  if (_callback != nullptr) {
    const NSPoint origin =
        NSMakePoint(_centerScreenPoint.x - _visualSize / 2.0,
                    _centerScreenPoint.y - _visualSize / 2.0);
    _callback(kPetCallbackMoved, nullptr, origin.x, origin.y, _displayID);
  }
}

- (NSURL *)supportedURLFromDraggingInfo:(id<NSDraggingInfo>)sender
                              fileKind:(uint32_t *)fileKind {
  NSArray<NSURL *> *urls = [sender.draggingPasteboard
      readObjectsForClasses:@[NSURL.class]
                    options:@{NSPasteboardURLReadingFileURLsOnlyKey : @YES}];
  if (urls.count == 0) return nil;
  NSURL *url = urls.firstObject;
  return PetURLIsRegularFile(url, fileKind) ? url : nil;
}

- (NSDragOperation)dragEnteredWithURL:(NSURL *)url
                            fileKind:(uint32_t)fileKind {
  if (url == nil || _dropSession.waitingForAck()) return NSDragOperationNone;
  const uint64_t generation =
      _dropSession.enter(url.path.fileSystemRepresentation, fileKind);
  if (generation == 0) return NSDragOperationNone;
  if (_callback != nullptr) {
    _callback(kPetCallbackDropEntered, nullptr, 0, 0, _displayID);
  }
  [self setDropHovering:YES];
  return NSDragOperationCopy;
}

- (BOOL)performDropWithURL:(NSURL *)url fileKind:(uint32_t)fileKind {
  if (url == nil || fileKind != _dropSession.fileKind()) return NO;
  const uint64_t generation = _dropSession.generation();
  const char *path = url.path.fileSystemRepresentation;
  if (!_dropSession.submit(generation, path)) return NO;
  [self setDropHovering:NO];
  _pendingDropSupported =
      fileKind == PET_FILE_3MF || fileKind == PET_FILE_GCODE;
  _pendingDropGeneration = generation;
  _pendingDropPath = [url.path copy];
  _ingestStartedAt = CFAbsoluteTimeGetCurrent();
  [self setIngestProgress:0 ejectProgress:0];
  [_ingestTimer invalidate];
  __weak BHPetHost *weakSelf = self;
  _ingestTimer =
      [NSTimer timerWithTimeInterval:1.0 / 60.0
                             repeats:YES
                               block:^(__unused NSTimer *timer) {
                                 BHPetHost *strongSelf = weakSelf;
                                 if (strongSelf) {
                                   [strongSelf tickIngestAnimation];
                                 }
                               }];
  [NSRunLoop.mainRunLoop addTimer:_ingestTimer
                         forMode:NSRunLoopCommonModes];
  return YES;
}

- (void)setIngestProgress:(CGFloat)ingestProgress
            ejectProgress:(CGFloat)ejectProgress {
  _ingestProgress = MIN(MAX(ingestProgress, 0.0), 1.0);
  _ejectProgress = MIN(MAX(ejectProgress, 0.0), 1.0);
  for (BHPetPane *pane in _panes) {
    pane.blackHoleView.blackHoleIngestProgress = _ingestProgress;
    pane.blackHoleView.blackHoleEjectProgress = _ejectProgress;
  }
}

- (void)setDropHovering:(BOOL)hovering {
  if (_dropHovering == hovering) return;
  _dropHovering = hovering;
  [self updatePanes];
}

- (void)tickIngestAnimation {
  if (_pendingDropGeneration == 0 || _pendingDropPath == nil) {
    [_ingestTimer invalidate];
    _ingestTimer = nil;
    [self setIngestProgress:0 ejectProgress:0];
    return;
  }
  const CFTimeInterval elapsed =
      CFAbsoluteTimeGetCurrent() - _ingestStartedAt;
  if (elapsed < kPetSwallowDurationSeconds) {
    [self setIngestProgress:PetSwallowProgress(elapsed)
              ejectProgress:0];
    return;
  }
  if (!_pendingDropSupported &&
      elapsed <
          kPetSwallowDurationSeconds + kPetEjectDurationSeconds) {
    [self setIngestProgress:1
              ejectProgress:PetEjectProgress(elapsed)];
    return;
  }

  [_ingestTimer invalidate];
  _ingestTimer = nil;
  const uint64_t generation = _pendingDropGeneration;
  if (_pendingDropSupported && _callback != nullptr) {
    [self setIngestProgress:1 ejectProgress:0];
    _callback(kPetCallbackFileDropped,
              _pendingDropPath.fileSystemRepresentation, 0, 0, generation);
    return;
  }

  _dropSession.finish(generation, PET_DROP_REJECTED);
  _pendingDropGeneration = 0;
  _pendingDropPath = nil;
  _pendingDropSupported = NO;
  [_targetView finishIngestAnimation];
  [self setIngestProgress:0 ejectProgress:0];
}

- (void)dragExited {
  [self setDropHovering:NO];
  _dropSession.cancelHover();
  if (_callback != nullptr) {
    _callback(kPetCallbackDropExited, nullptr, 0, 0, _displayID);
  }
}

- (void)finishDrop:(uint64_t)generation result:(uint32_t)result {
  if (_dropSession.finish(generation, result)) {
    [self setDropHovering:NO];
    _pendingDropGeneration = 0;
    _pendingDropPath = nil;
    _pendingDropSupported = NO;
    _targetView.fileHovering = NO;
    [_targetView finishIngestAnimation];
    [self setIngestProgress:0 ejectProgress:0];
  }
}

- (void)requestCapturePermission {
  if (@available(macOS 14.0, *)) {
    if (!CGPreflightScreenCaptureAccess()) {
      CGRequestScreenCaptureAccess();
    }
  }
  [self emitPermissionState];
}

- (uint32_t)captureState {
  if (@available(macOS 14.0, *)) {
    return CGPreflightScreenCaptureAccess() ? PET_CAPTURE_READY
                                            : PET_CAPTURE_DENIED;
  }
  return PET_CAPTURE_UNAVAILABLE;
}

- (void)emitPermissionState {
  if (_callback == nullptr) return;
  const uint32_t state = [self captureState];
  const char *payload = state == PET_CAPTURE_READY
                            ? "ready"
                            : (state == PET_CAPTURE_DENIED ? "denied"
                                                          : "unavailable");
  _callback(kPetCallbackPermissionChanged, payload, 0, 0, _displayID);
}

- (uint32_t)rendererState {
  for (BHPetPane *pane in _panes) {
    if (pane.blackHoleView.rendererAvailable) return PET_RENDERER_READY;
  }
  return PET_RENDERER_UNAVAILABLE;
}

- (void)screenParametersChanged:(NSNotification *)notification {
  (void)notification;
  if (_lifecycle.destroyed()) return;
  const std::vector<PetScreenFrame> frames = PetScreenFrames();
  if (frames.empty()) return;
  const PetScreenPoint recovered = PetRecoverCenter(
      {_centerScreenPoint.x, _centerScreenPoint.y}, frames.data(),
      frames.size());
  _centerScreenPoint = NSMakePoint(recovered.x, recovered.y);
  [self selectDisplay];
  [self rebuildPanes];
  [self syncTargetFrame];
  if (_callback != nullptr) {
    const NSPoint origin =
        NSMakePoint(_centerScreenPoint.x - _visualSize / 2.0,
                    _centerScreenPoint.y - _visualSize / 2.0);
    _callback(kPetCallbackDisplayChanged, nullptr, origin.x, origin.y,
              _displayID);
  }
}

- (void)workspaceWillSleep:(NSNotification *)notification {
  (void)notification;
  if (_lifecycle.destroyed() || _lifecycle.sleeping()) return;
  _lifecycle.sleep();
  [self updatePanes];
  if (_callback != nullptr) {
    _callback(kPetCallbackSleep, nullptr, 0, 0, _displayID);
  }
}

- (void)workspaceDidWake:(NSNotification *)notification {
  (void)notification;
  if (_lifecycle.destroyed() || !_lifecycle.sleeping()) return;
  _lifecycle.wake();
  [self screenParametersChanged:nil];
  [self updatePanes];
  if (_callback != nullptr) {
    _callback(kPetCallbackWake, nullptr, 0, 0, _displayID);
  }
}

- (uint32_t)shutdown {
  if (_lifecycle.destroyed()) return PET_SHUTDOWN_COMPLETE;
  _lifecycle.destroy();
  [NSNotificationCenter.defaultCenter removeObserver:self];
  [NSWorkspace.sharedWorkspace.notificationCenter removeObserver:self];
  _targetView.host = nil;
  [_targetPanel orderOut:nil];
  [_targetPanel close];
  for (BHPetPane *pane in _panes) {
    [pane.blackHoleView setCaptureEnabled:NO];
    [pane.blackHoleView setRenderingPaused:YES];
    [pane.panel orderOut:nil];
    [pane.panel close];
  }
  [_panes removeAllObjects];
  _callback = nullptr;
  return PET_SHUTDOWN_COMPLETE;
}

@end

@interface BHPetBridge : NSObject
@property(nonatomic, assign) PetCallback callback;
@property(nonatomic, copy) NSString *metalSource;
@property(nonatomic, strong) BHPetHost *host;
@property(nonatomic) BOOL destroyed;
@end

@implementation BHPetBridge {
  PetApplyGenerationGate _applyGate;
}

- (instancetype)initWithCallback:(PetCallback)callback
                      metalSource:(NSString *)metalSource {
  self = [super init];
  if (self) {
    self.callback = callback;
    self.metalSource = metalSource;
  }
  return self;
}

- (void)ensureHost {
  if (self.host == nil && !self.destroyed) {
    self.host = [[BHPetHost alloc] initWithCallback:self.callback
                                        metalSource:self.metalSource];
  }
}

- (uint64_t)issueApplyGeneration { return _applyGate.issue(); }
- (BOOL)acceptApplyGeneration:(uint64_t)generation {
  return _applyGate.accept(generation);
}

- (uint32_t)shutdown {
  if (self.destroyed) return PET_SHUTDOWN_COMPLETE;
  self.destroyed = YES;
  const uint32_t result =
      self.host == nil ? PET_SHUTDOWN_COMPLETE : [self.host shutdown];
  self.host = nil;
  self.metalSource = nil;
  self.callback = nullptr;
  return result;
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
  if (NSThread.isMainThread || NSApp == nil) {
    block();
  } else {
    dispatch_sync(dispatch_get_main_queue(), block);
  }
}

static BOOL PetConfigIsValid(PetConfig config) {
  const BOOL validFps =
      config.fps == 0 || config.fps == 30 || config.fps == 60;
  return config.abi_version == kPetAbiVersion && config.mode <= 1 &&
         config.effective_mode <= 1 && config.has_position <= 1 &&
         std::isfinite(config.size) && config.size >= kPetMinimumSize &&
         config.size <= kPetMaximumSize &&
         (!config.has_position ||
          (std::isfinite(config.x) && std::isfinite(config.y))) &&
         validFps && config.visible <= 1 && config.reduce_motion <= 1 &&
         config.request_permission <= 1 && config.visual_style <= 1;
}

extern "C" void *pet_create(PetCallback callback,
                             const char *metal_source) {
  NSString *source =
      metal_source == nullptr
          ? @""
          : [[NSString alloc] initWithUTF8String:metal_source];
  if (source == nil) return nullptr;
  BHPetBridge *bridge = [[BHPetBridge alloc] initWithCallback:callback
                                                  metalSource:source];
  void *handle = (__bridge_retained void *)bridge;
  RunOnMain(^{
    [bridge ensureHost];
  });
  return handle;
}

extern "C" uint32_t pet_destroy(void *handle) {
  if (handle == nullptr) return PET_SHUTDOWN_COMPLETE;
  BHPetBridge *bridge = (__bridge_transfer BHPetBridge *)handle;
  __block uint32_t result = PET_SHUTDOWN_COMPLETE;
  RunOnMainAndWait(^{
    result = [bridge shutdown];
  });
  return result;
}

extern "C" bool pet_apply(void *handle, PetConfig config) {
  if (handle == nullptr || !PetConfigIsValid(config)) return false;
  BHPetBridge *bridge = (__bridge BHPetBridge *)handle;
  const uint64_t generation = [bridge issueApplyGeneration];
  RunOnMain(^{
    if (!bridge.destroyed && [bridge acceptApplyGeneration:generation]) {
      [bridge ensureHost];
      [bridge.host applyConfig:config];
    }
  });
  return true;
}

extern "C" void pet_show(void *handle) {
  if (handle == nullptr) return;
  BHPetBridge *bridge = (__bridge BHPetBridge *)handle;
  RunOnMain(^{
    if (!bridge.destroyed) {
      [bridge ensureHost];
      [bridge.host show];
    }
  });
}

extern "C" void pet_hide(void *handle) {
  if (handle == nullptr) return;
  BHPetBridge *bridge = (__bridge BHPetBridge *)handle;
  RunOnMain(^{
    if (!bridge.destroyed) [bridge.host hide];
  });
}

extern "C" void pet_reset(void *handle) {
  if (handle == nullptr) return;
  BHPetBridge *bridge = (__bridge BHPetBridge *)handle;
  RunOnMain(^{
    if (!bridge.destroyed) {
      [bridge ensureHost];
      [bridge.host reset];
    }
  });
}

extern "C" void pet_signal(void *handle, uint32_t signal) {
  if (handle == nullptr) return;
  BHPetBridge *bridge = (__bridge BHPetBridge *)handle;
  RunOnMain(^{
    if (!bridge.destroyed) [bridge.host signal:signal];
  });
}

extern "C" void pet_finish_drop(void *handle, uint64_t generation,
                                 uint32_t result) {
  if (handle == nullptr || generation == 0) return;
  BHPetBridge *bridge = (__bridge BHPetBridge *)handle;
  RunOnMain(^{
    if (!bridge.destroyed) {
      [bridge.host finishDrop:generation result:result];
    }
  });
}

extern "C" uint32_t pet_capture_state(void *handle) {
  if (handle == nullptr) return PET_CAPTURE_UNAVAILABLE;
  BHPetBridge *bridge = (__bridge BHPetBridge *)handle;
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
  if (handle == nullptr) return PET_RENDERER_UNAVAILABLE;
  BHPetBridge *bridge = (__bridge BHPetBridge *)handle;
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
