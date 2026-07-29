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
@property(nonatomic) BHStyle blackHoleStyle;
- (instancetype)initWithFrame:(NSRect)frame
                  metalSource:(NSString *)metalSource;
- (void)setCaptureEnabled:(BOOL)enabled;
- (void)setTargetFramesPerSecond:(NSInteger)fps;
- (void)refreshBackgroundNow;
@end
