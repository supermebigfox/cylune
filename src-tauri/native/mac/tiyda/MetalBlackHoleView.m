#import "BlackHoleDesktop.h"
#import "black_hole_params.h"
#import <MetalKit/MetalKit.h>
#import <ScreenCaptureKit/ScreenCaptureKit.h>
#import <simd/simd.h>

typedef struct {
    vector_float2 resolution;
    float time;
    float size;
    float brightness;
    float speed;
    uint32_t style;
    vector_float2 center;
} RenderParams;

@interface MetalBlackHoleView () <MTKViewDelegate>
@end

@implementation MetalBlackHoleView {
    MTKView *_metalView;
    id<MTLCommandQueue> _queue;
    id<MTLRenderPipelineState> _pipeline;
    MTKTextureLoader *_textureLoader;
    id<MTLTexture> _wallpaperTexture;
    id<MTLTexture> _screenTexture;
    SCContentFilter *_captureFilter;
    SCStreamConfiguration *_captureConfiguration;
    BOOL _captureConfigurationInFlight;
    BOOL _captureInFlight;
    BOOL _captureErrorLogged;
    BOOL _captureEnabledLogged;
    CFAbsoluteTime _lastCaptureTime;
    CFAbsoluteTime _startTime;
    BOOL _captureEnabled;
}

- (instancetype)initWithFrame:(NSRect)frame
                  metalSource:(NSString *)metalSource {
    id<MTLDevice> device = MTLCreateSystemDefaultDevice();
    self = [super initWithFrame:frame];
    if (!self || !device) return self;
    _blackHoleCenterInScreen = CGPointMake(NSMidX(frame), NSMidY(frame));
    _blackHoleSize = 420.0;
    _blackHoleBrightness = 1.0f;
    _blackHoleSpeed = 1.0f;
    _blackHoleStyle = BHStyleGargantua;
    _captureEnabled = YES;
    _startTime = CFAbsoluteTimeGetCurrent();
    self.wantsLayer = YES;
    self.layer.opaque = NO;

    _metalView = [[MTKView alloc] initWithFrame:self.bounds device:device];
    _metalView.autoresizingMask = NSViewWidthSizable | NSViewHeightSizable;
    _metalView.colorPixelFormat = MTLPixelFormatBGRA8Unorm;
    _metalView.clearColor = MTLClearColorMake(0, 0, 0, 0);
    _metalView.layer.opaque = NO;
    _metalView.paused = NO;
    _metalView.enableSetNeedsDisplay = NO;
    _metalView.preferredFramesPerSecond = 30;
    _metalView.delegate = self;
    [self addSubview:_metalView];

    _queue = [device newCommandQueue];
    _textureLoader = [[MTKTextureLoader alloc] initWithDevice:device];
    MTLTextureDescriptor *fallbackDescriptor = [MTLTextureDescriptor texture2DDescriptorWithPixelFormat:MTLPixelFormatBGRA8Unorm width:1 height:1 mipmapped:NO];
    _wallpaperTexture = [device newTextureWithDescriptor:fallbackDescriptor];
    uint32_t black = 0;
    [_wallpaperTexture replaceRegion:MTLRegionMake2D(0, 0, 1, 1) mipmapLevel:0 withBytes:&black bytesPerRow:sizeof(black)];
    NSError *error = nil;
    id<MTLLibrary> library = [device newLibraryWithSource:metalSource options:nil error:&error];
    if (!library) { NSLog(@"Black Hole: Metal shader compile failed: %@", error); return self; }
    MTLRenderPipelineDescriptor *descriptor = [MTLRenderPipelineDescriptor new];
    descriptor.vertexFunction = [library newFunctionWithName:@"blackHoleVertex"];
    descriptor.fragmentFunction = [library newFunctionWithName:@"blackHoleFragment"];
    descriptor.colorAttachments[0].pixelFormat = _metalView.colorPixelFormat;
    descriptor.colorAttachments[0].blendingEnabled = YES;
    descriptor.colorAttachments[0].sourceRGBBlendFactor = MTLBlendFactorSourceAlpha;
    descriptor.colorAttachments[0].destinationRGBBlendFactor = MTLBlendFactorOneMinusSourceAlpha;
    _pipeline = [device newRenderPipelineStateWithDescriptor:descriptor error:&error];
    if (!_pipeline) NSLog(@"Black Hole: Metal pipeline creation failed: %@", error);
    return self;
}

- (BOOL)isOpaque { return NO; }

- (void)setCaptureEnabled:(BOOL)enabled {
    _captureEnabled = enabled;
    if (!enabled) {
        _screenTexture = nil;
        _captureFilter = nil;
        _captureConfiguration = nil;
    }
}

- (void)setTargetFramesPerSecond:(NSInteger)fps {
    _metalView.preferredFramesPerSecond = MAX(1, fps);
}

- (void)refreshBackgroundNow {
    _lastCaptureTime = 0;
    [self refreshScreenTextureIfNeeded];
}

- (void)viewDidMoveToWindow {
    [super viewDidMoveToWindow];
    NSScreen *screen = self.window.screen;
    NSURL *wallpaperURL = screen ? [NSWorkspace.sharedWorkspace desktopImageURLForScreen:screen] : nil;
    if (!wallpaperURL) return;
    NSError *textureError = nil;
    id<MTLTexture> texture = [_textureLoader newTextureWithContentsOfURL:wallpaperURL
                                                                 options:@{
                                                                     MTKTextureLoaderOptionSRGB: @NO,
                                                                     MTKTextureLoaderOptionOrigin: MTKTextureLoaderOriginTopLeft,
                                                                 }
                                                                   error:&textureError];
    if (texture) {
        _wallpaperTexture = texture;
    } else {
        NSLog(@"Black Hole: wallpaper texture error: %@", textureError);
    }
}

- (void)configureScreenCaptureIfNeeded {
    if (_captureFilter || _captureConfigurationInFlight || !_captureEnabled || !CGPreflightScreenCaptureAccess()) return;
    _captureConfigurationInFlight = YES;
    __weak MetalBlackHoleView *weakSelf = self;
    [SCShareableContent getShareableContentWithCompletionHandler:^(SCShareableContent *content, NSError *error) {
        MetalBlackHoleView *strongSelf = weakSelf;
        if (!strongSelf) return;
        dispatch_async(dispatch_get_main_queue(), ^{
            strongSelf->_captureConfigurationInFlight = NO;
            if (error) {
                if (!strongSelf->_captureErrorLogged) NSLog(@"Black Hole: screen background unavailable: %@", error);
                strongSelf->_captureErrorLogged = YES;
                return;
            }
            NSScreen *screen = strongSelf.window.screen;
            NSNumber *screenNumber = screen.deviceDescription[@"NSScreenNumber"];
            CGDirectDisplayID displayID = (CGDirectDisplayID)screenNumber.unsignedIntValue;
            SCDisplay *display = nil;
            for (SCDisplay *candidate in content.displays) {
                if (candidate.displayID == displayID) { display = candidate; break; }
            }
            if (!display) return;

            NSMutableArray<SCWindow *> *ownWindows = [NSMutableArray array];
            pid_t processID = NSProcessInfo.processInfo.processIdentifier;
            for (SCWindow *candidate in content.windows) {
                if (candidate.owningApplication.processID == processID) [ownWindows addObject:candidate];
            }
            strongSelf->_captureFilter = [[SCContentFilter alloc] initWithDisplay:display excludingWindows:ownWindows];
            SCStreamConfiguration *configuration = [SCStreamConfiguration new];
            configuration.width = (size_t)round(NSWidth(screen.frame));
            configuration.height = (size_t)round(NSHeight(screen.frame));
            configuration.showsCursor = NO;
            strongSelf->_captureConfiguration = configuration;
        });
    }];
}

- (void)refreshScreenTextureIfNeeded {
    if (!_captureEnabled || !CGPreflightScreenCaptureAccess()) return;
    if (!_captureFilter) {
        [self configureScreenCaptureIfNeeded];
        return;
    }
    CFAbsoluteTime now = CFAbsoluteTimeGetCurrent();
    if (_captureInFlight || now - _lastCaptureTime < 0.10) return;
    _captureInFlight = YES;
    _lastCaptureTime = now;
    __weak MetalBlackHoleView *weakSelf = self;
    [SCScreenshotManager captureImageWithFilter:_captureFilter configuration:_captureConfiguration completionHandler:^(CGImageRef image, NSError *error) {
        MetalBlackHoleView *strongSelf = weakSelf;
        if (!strongSelf) return;
        CGImageRef retainedImage = image ? CGImageRetain(image) : nil;
        dispatch_async(dispatch_get_main_queue(), ^{
            strongSelf->_captureInFlight = NO;
            if (!retainedImage) {
                if (!strongSelf->_captureErrorLogged) NSLog(@"Black Hole: screen background capture failed: %@", error);
                strongSelf->_captureErrorLogged = YES;
                return;
            }
            NSError *textureError = nil;
            id<MTLTexture> texture = [strongSelf->_textureLoader newTextureWithCGImage:retainedImage
                                                                               options:@{
                                                                                   MTKTextureLoaderOptionSRGB: @NO,
                                                                                   MTKTextureLoaderOptionOrigin: MTKTextureLoaderOriginTopLeft,
                                                                               }
                                                                                 error:&textureError];
            CGImageRelease(retainedImage);
            if (texture) {
                strongSelf->_screenTexture = texture;
                strongSelf->_captureErrorLogged = NO;
                if (!strongSelf->_captureEnabledLogged) NSLog(@"Black Hole: live application background enabled");
                strongSelf->_captureEnabledLogged = YES;
            } else if (!strongSelf->_captureErrorLogged) {
                NSLog(@"Black Hole: screen texture error: %@", textureError);
                strongSelf->_captureErrorLogged = YES;
            }
        });
    }];
}

- (void)drawInMTKView:(MTKView *)view {
    [self refreshScreenTextureIfNeeded];
    id<MTLTexture> backgroundTexture = _captureEnabled && _screenTexture ? _screenTexture : _wallpaperTexture;
    if (!_pipeline || !backgroundTexture || !view.currentDrawable || !view.currentRenderPassDescriptor) return;
    NSScreen *screen = self.window.screen;
    NSRect screenFrame = screen ? screen.frame : self.window.frame;
    CGFloat centerX = NSWidth(screenFrame) > 0
                          ? (_blackHoleCenterInScreen.x - NSMinX(screenFrame)) /
                                NSWidth(screenFrame)
                          : 0.5;
    CGFloat centerY = NSHeight(screenFrame) > 0
                          ? 1.0 -
                                (_blackHoleCenterInScreen.y - NSMinY(screenFrame)) /
                                    NSHeight(screenFrame)
                          : 0.5;
    RenderParams params = {
        .resolution = {(float)view.drawableSize.width, (float)view.drawableSize.height},
        .time = (float)(CFAbsoluteTimeGetCurrent() - _startTime),
        .size = BHShaderSizeForPixels(
            (float)(_blackHoleSize * (screen ? screen.backingScaleFactor : 1.0)),
            (float)view.drawableSize.height),
        .brightness = _blackHoleBrightness,
        .speed = _blackHoleSpeed,
        .style = (uint32_t)_blackHoleStyle,
        .center = {(float)centerX, (float)centerY},
    };
    id<MTLCommandBuffer> command = [_queue commandBuffer];
    id<MTLRenderCommandEncoder> encoder = [command renderCommandEncoderWithDescriptor:view.currentRenderPassDescriptor];
    [encoder setRenderPipelineState:_pipeline];
    [encoder setFragmentTexture:backgroundTexture atIndex:0];
    [encoder setFragmentBytes:&params length:sizeof(params) atIndex:0];
    [encoder drawPrimitives:MTLPrimitiveTypeTriangleStrip vertexStart:0 vertexCount:4];
    [encoder endEncoding];
    [command presentDrawable:view.currentDrawable];
    [command commit];
}

- (void)mtkView:(MTKView *)view drawableSizeWillChange:(CGSize)size { }
@end
