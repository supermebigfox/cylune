#import <Cocoa/Cocoa.h>

typedef NS_ENUM(uint32_t, BHStyle) {
  BHStyleDefault = 0,
  BHStyleGargantua = 1,
};

@interface MetalBlackHoleView : NSView
@property(nonatomic) CGPoint blackHoleCenterInScreen;
@property(nonatomic) CGFloat blackHoleSize;
@property(nonatomic) float blackHoleBrightness;
@property(nonatomic) float blackHoleSpeed;
@property(nonatomic) float blackHolePullGain;
@property(nonatomic) float blackHoleIngestProgress;
@property(nonatomic) float blackHoleEjectProgress;
@property(nonatomic) BHStyle blackHoleStyle;
@property(nonatomic, readonly) BOOL rendererAvailable;
- (instancetype)initWithFrame:(NSRect)frame
                  metalSource:(NSString *)metalSource;
- (void)setCaptureEnabled:(BOOL)enabled;
- (void)setTargetFramesPerSecond:(NSInteger)fps;
- (void)setRenderingPaused:(BOOL)paused;
- (void)refreshBackgroundNow;
@end
