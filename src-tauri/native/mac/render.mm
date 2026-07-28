#import "bridge.h"

#import <AppKit/AppKit.h>
#import <Metal/Metal.h>
#import <QuartzCore/CAMetalLayer.h>
#import <simd/simd.h>

#include <algorithm>
#include <stddef.h>

static_assert(sizeof(PetRenderUniforms) == 48,
              "PetRenderUniforms ABI size changed");
static_assert(offsetof(PetRenderUniforms, viewport_px) == 0,
              "PetRenderUniforms viewport offset changed");
static_assert(offsetof(PetRenderUniforms, time_seconds) == 8,
              "PetRenderUniforms time offset changed");
static_assert(offsetof(PetRenderUniforms, pending_count) == 32,
              "PetRenderUniforms pending offset changed");

@interface BPPetMetalRenderer : NSObject
- (instancetype)initWithSource:(NSString *)source
                         layer:(CAMetalLayer *)layer;
- (BOOL)drawSurface:(IOSurfaceRef)surface
           uniforms:(PetRenderUniforms)uniforms;
- (BOOL)drawBytes:(const uint8_t *)bytes
            width:(uint32_t)width
           height:(uint32_t)height
             mode:(uint32_t)mode
           output:(uint8_t *)output
         capacity:(uint64_t)capacity;
@end

@implementation BPPetMetalRenderer {
  id<MTLDevice> _device;
  id<MTLCommandQueue> _commandQueue;
  id<MTLRenderPipelineState> _pipeline;
  id<MTLSamplerState> _sampler;
  id<MTLTexture> _transparentTexture;
  __weak CAMetalLayer *_layer;
}

- (instancetype)initWithSource:(NSString *)source
                         layer:(CAMetalLayer *)layer {
  self = [super init];
  if (self) {
    _device = MTLCreateSystemDefaultDevice();
    if (_device == nil || source.length == 0) {
      return nil;
    }
    NSError *libraryError = nil;
    id<MTLLibrary> library =
        [_device newLibraryWithSource:source options:nil error:&libraryError];
    if (library == nil || libraryError != nil) {
      NSLog(@"pet Metal library failed: %@", libraryError);
      return nil;
    }
    id<MTLFunction> vertex = [library newFunctionWithName:@"pet_vertex"];
    id<MTLFunction> fragment = [library newFunctionWithName:@"pet_fragment"];
    if (vertex == nil || fragment == nil) {
      return nil;
    }
    MTLRenderPipelineDescriptor *descriptor =
        [[MTLRenderPipelineDescriptor alloc] init];
    descriptor.vertexFunction = vertex;
    descriptor.fragmentFunction = fragment;
    descriptor.colorAttachments[0].pixelFormat = MTLPixelFormatBGRA8Unorm;
    descriptor.colorAttachments[0].blendingEnabled = YES;
    descriptor.colorAttachments[0].sourceRGBBlendFactor =
        MTLBlendFactorOne;
    descriptor.colorAttachments[0].destinationRGBBlendFactor =
        MTLBlendFactorOneMinusSourceAlpha;
    descriptor.colorAttachments[0].sourceAlphaBlendFactor =
        MTLBlendFactorOne;
    descriptor.colorAttachments[0].destinationAlphaBlendFactor =
        MTLBlendFactorOneMinusSourceAlpha;
    NSError *pipelineError = nil;
    _pipeline = [_device newRenderPipelineStateWithDescriptor:descriptor
                                                        error:&pipelineError];
    if (_pipeline == nil || pipelineError != nil) {
      NSLog(@"pet Metal pipeline failed: %@", pipelineError);
      return nil;
    }
    _commandQueue = [_device newCommandQueue];
    if (_commandQueue == nil) {
      return nil;
    }
    MTLSamplerDescriptor *samplerDescriptor =
        [[MTLSamplerDescriptor alloc] init];
    samplerDescriptor.minFilter = MTLSamplerMinMagFilterLinear;
    samplerDescriptor.magFilter = MTLSamplerMinMagFilterLinear;
    samplerDescriptor.sAddressMode = MTLSamplerAddressModeClampToEdge;
    samplerDescriptor.tAddressMode = MTLSamplerAddressModeClampToEdge;
    _sampler = [_device newSamplerStateWithDescriptor:samplerDescriptor];

    MTLTextureDescriptor *transparentDescriptor =
        [MTLTextureDescriptor
            texture2DDescriptorWithPixelFormat:MTLPixelFormatBGRA8Unorm
                                         width:1
                                        height:1
                                     mipmapped:NO];
    transparentDescriptor.usage = MTLTextureUsageShaderRead;
    _transparentTexture =
        [_device newTextureWithDescriptor:transparentDescriptor];
    const uint32_t transparent = 0;
    [_transparentTexture
        replaceRegion:MTLRegionMake2D(0, 0, 1, 1)
          mipmapLevel:0
            withBytes:&transparent
          bytesPerRow:sizeof(transparent)];
    _layer = layer;
    if (layer != nil) {
      layer.device = _device;
      layer.pixelFormat = MTLPixelFormatBGRA8Unorm;
      layer.framebufferOnly = YES;
      layer.opaque = NO;
      layer.backgroundColor = NSColor.clearColor.CGColor;
    }
  }
  return self;
}

- (id<MTLTexture>)textureForSurface:(IOSurfaceRef)surface {
  if (surface == nullptr) {
    return _transparentTexture;
  }
  const size_t width = IOSurfaceGetWidth(surface);
  const size_t height = IOSurfaceGetHeight(surface);
  if (width == 0 || height == 0) {
    return _transparentTexture;
  }
  MTLTextureDescriptor *descriptor =
      [MTLTextureDescriptor
          texture2DDescriptorWithPixelFormat:MTLPixelFormatBGRA8Unorm
                                       width:width
                                      height:height
                                   mipmapped:NO];
  descriptor.usage = MTLTextureUsageShaderRead;
  return [_device newTextureWithDescriptor:descriptor
                                 iosurface:surface
                                     plane:0];
}

- (BOOL)encodeToTexture:(id<MTLTexture>)output
                capture:(id<MTLTexture>)capture
               uniforms:(PetRenderUniforms)uniforms
          commandBuffer:(id<MTLCommandBuffer>)commandBuffer {
  if (output == nil || capture == nil || commandBuffer == nil) {
    return NO;
  }
  MTLRenderPassDescriptor *pass = [MTLRenderPassDescriptor renderPassDescriptor];
  pass.colorAttachments[0].texture = output;
  pass.colorAttachments[0].loadAction = MTLLoadActionClear;
  pass.colorAttachments[0].storeAction = MTLStoreActionStore;
  pass.colorAttachments[0].clearColor = MTLClearColorMake(0.0, 0.0, 0.0, 0.0);
  id<MTLRenderCommandEncoder> encoder =
      [commandBuffer renderCommandEncoderWithDescriptor:pass];
  if (encoder == nil) {
    return NO;
  }
  [encoder setRenderPipelineState:_pipeline];
  [encoder setFragmentTexture:capture atIndex:0];
  [encoder setFragmentSamplerState:_sampler atIndex:0];
  [encoder setFragmentBytes:&uniforms
                     length:sizeof(uniforms)
                    atIndex:0];
  [encoder drawPrimitives:MTLPrimitiveTypeTriangle
              vertexStart:0
              vertexCount:3];
  [encoder endEncoding];
  return YES;
}

- (BOOL)drawSurface:(IOSurfaceRef)surface
           uniforms:(PetRenderUniforms)uniforms {
  CAMetalLayer *layer = _layer;
  if (layer == nil) {
    return NO;
  }
  id<CAMetalDrawable> drawable = [layer nextDrawable];
  if (drawable == nil) {
    return NO;
  }
  id<MTLCommandBuffer> commandBuffer = [_commandQueue commandBuffer];
  if (![self encodeToTexture:drawable.texture
                     capture:[self textureForSurface:surface]
                    uniforms:uniforms
               commandBuffer:commandBuffer]) {
    return NO;
  }
  [commandBuffer presentDrawable:drawable];
  [commandBuffer commit];
  return YES;
}

- (BOOL)drawBytes:(const uint8_t *)bytes
            width:(uint32_t)width
           height:(uint32_t)height
             mode:(uint32_t)mode
           output:(uint8_t *)output
         capacity:(uint64_t)capacity {
  const uint64_t required =
      (uint64_t)width * (uint64_t)height * (uint64_t)4;
  if (width == 0 || height == 0 || output == nullptr ||
      capacity < required || (mode == 0 && bytes == nullptr)) {
    return NO;
  }
  id<MTLTexture> capture = _transparentTexture;
  if (bytes != nullptr) {
    MTLTextureDescriptor *captureDescriptor =
        [MTLTextureDescriptor
            texture2DDescriptorWithPixelFormat:MTLPixelFormatBGRA8Unorm
                                         width:width
                                        height:height
                                     mipmapped:NO];
    captureDescriptor.usage = MTLTextureUsageShaderRead;
    capture = [_device newTextureWithDescriptor:captureDescriptor];
    [capture replaceRegion:MTLRegionMake2D(0, 0, width, height)
               mipmapLevel:0
                 withBytes:bytes
               bytesPerRow:(NSUInteger)width * 4];
  }
  MTLTextureDescriptor *outputDescriptor =
      [MTLTextureDescriptor
          texture2DDescriptorWithPixelFormat:MTLPixelFormatBGRA8Unorm
                                       width:width
                                      height:height
                                   mipmapped:NO];
  outputDescriptor.usage =
      MTLTextureUsageRenderTarget | MTLTextureUsageShaderRead;
  outputDescriptor.storageMode = MTLStorageModeShared;
  id<MTLTexture> outputTexture =
      [_device newTextureWithDescriptor:outputDescriptor];
  PetRenderUniforms uniforms = {};
  uniforms.viewport_px[0] = (float)width;
  uniforms.viewport_px[1] = (float)height;
  uniforms.lens_strength = 1.0f;
  uniforms.mode = mode;
  id<MTLCommandBuffer> commandBuffer = [_commandQueue commandBuffer];
  if (![self encodeToTexture:outputTexture
                     capture:capture
                    uniforms:uniforms
               commandBuffer:commandBuffer]) {
    return NO;
  }
  [commandBuffer commit];
  [commandBuffer waitUntilCompleted];
  if (commandBuffer.status != MTLCommandBufferStatusCompleted) {
    return NO;
  }
  [outputTexture getBytes:output
              bytesPerRow:(NSUInteger)width * 4
               fromRegion:MTLRegionMake2D(0, 0, width, height)
              mipmapLevel:0];
  return YES;
}

@end

extern "C" void *mac_renderer_create(const char *metal_source,
                                      void *metal_layer) {
  if (metal_source == nullptr) {
    return nullptr;
  }
  NSString *source = [NSString stringWithUTF8String:metal_source];
  CAMetalLayer *layer =
      metal_layer == nullptr ? nil : (__bridge CAMetalLayer *)metal_layer;
  BPPetMetalRenderer *renderer =
      [[BPPetMetalRenderer alloc] initWithSource:source layer:layer];
  return renderer == nil ? nullptr : (__bridge_retained void *)renderer;
}

extern "C" void mac_renderer_destroy(void *handle) {
  if (handle != nullptr) {
    (void)(__bridge_transfer BPPetMetalRenderer *)handle;
  }
}

extern "C" bool mac_renderer_draw(void *handle, IOSurfaceRef surface,
                                  PetRenderUniforms uniforms) {
  if (handle == nullptr) {
    return false;
  }
  return [(__bridge BPPetMetalRenderer *)handle drawSurface:surface
                                                   uniforms:uniforms];
}

extern "C" uint64_t pet_test_render_rgba(
    const uint8_t *input, uint32_t width, uint32_t height, uint32_t mode,
    uint8_t *output, uint64_t output_capacity, const char *metal_source) {
  void *handle = mac_renderer_create(metal_source, nullptr);
  if (handle == nullptr) {
    return 0;
  }
  const BOOL rendered =
      [(__bridge BPPetMetalRenderer *)handle drawBytes:input
                                                width:width
                                               height:height
                                                 mode:mode
                                               output:output
                                             capacity:output_capacity];
  mac_renderer_destroy(handle);
  if (!rendered) {
    return 0;
  }
  uint64_t checksum = 1469598103934665603ULL;
  const uint64_t length = (uint64_t)width * (uint64_t)height * 4ULL;
  for (uint64_t index = 0; index < length; ++index) {
    checksum = (checksum ^ output[index]) * 1099511628211ULL;
  }
  return checksum == 0 ? 1 : checksum;
}
