#import <Cocoa/Cocoa.h>

@interface VisualSettings : NSObject
@property CGFloat size;
@property CGFloat brightness;
@property CGFloat speed;
@property NSInteger style;
@property NSTimeInterval startTime;
@property BOOL alwaysOnTop;
@end

@interface MetalBlackHoleView : NSView
- (instancetype)initWithFrame:(NSRect)frame settings:(VisualSettings *)settings;
@end
