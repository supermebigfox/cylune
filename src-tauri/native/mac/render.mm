#import "bridge.h"
#import "pet_visual_state.h"

#import <AppKit/AppKit.h>
#import <Metal/Metal.h>
#import <QuartzCore/CAMetalLayer.h>
#import <simd/simd.h>

#include <algorithm>
#include <stddef.h>
#include <vector>

static_assert(sizeof(PetRenderUniforms) == 160,
              "PetRenderUniforms ABI size changed");
static_assert(offsetof(PetRenderUniforms, capture_origin_uv) == 8,
              "PetRenderUniforms capture origin offset changed");
static_assert(offsetof(PetRenderUniforms, center_uv) == 24,
              "PetRenderUniforms center offset changed");
static_assert(offsetof(PetRenderUniforms, temperature) == 40,
              "PetRenderUniforms temperature offset changed");
static_assert(offsetof(PetRenderUniforms, drop_origin_uv) == 104,
              "PetRenderUniforms drop origin offset changed");
static_assert(offsetof(PetRenderUniforms, pending_count) == 128,
              "PetRenderUniforms pending offset changed");
static_assert(offsetof(PetRenderUniforms, visual_style) == 148,
              "PetRenderUniforms visual style offset changed");
static_assert(offsetof(PetRenderUniforms, impact_level) == 152,
              "PetRenderUniforms impact offset changed");
static_assert(offsetof(PetRenderUniforms, feed_strength) == 156,
              "PetRenderUniforms feed offset changed");
static_assert(sizeof(PetRenderStats) == 16,
              "PetRenderStats ABI size changed");

struct PetPendingInstance {
  simd_float2 center;
  float diameter;
  float padding;
};

static_assert(sizeof(PetPendingInstance) == 16,
              "PetPendingInstance layout changed");

@interface BPPetMetalRenderer : NSObject
- (instancetype)initWithSource:(NSString *)source
                         layer:(CAMetalLayer *)layer;
- (uint32_t)drawSurface:(IOSurfaceRef)surface
               uniforms:(PetRenderUniforms)uniforms;
- (BOOL)drawBytes:(const uint8_t *)bytes
            width:(uint32_t)width
           height:(uint32_t)height
         uniforms:(PetRenderUniforms)uniforms
           output:(uint8_t *)output
         capacity:(uint64_t)capacity
            stats:(PetRenderStats *)stats;
@end

@implementation BPPetMetalRenderer {
  id<MTLDevice> _device;
  id<MTLCommandQueue> _commandQueue;
  id<MTLRenderPipelineState> _pipeline;
  id<MTLRenderPipelineState> _pendingPipeline;
  id<MTLSamplerState> _sampler;
  id<MTLTexture> _transparentTexture;
  __weak CAMetalLayer *_layer;
  id<MTLBuffer> _pendingInstances;
  uint32_t _pendingInstanceCount;
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
    id<MTLFunction> pendingVertex =
        [library newFunctionWithName:@"pet_pending_vertex"];
    id<MTLFunction> pendingFragment =
        [library newFunctionWithName:@"pet_pending_fragment"];
    if (pendingVertex == nil || pendingFragment == nil) {
      return nil;
    }
    MTLRenderPipelineDescriptor *pendingDescriptor =
        [[MTLRenderPipelineDescriptor alloc] init];
    pendingDescriptor.vertexFunction = pendingVertex;
    pendingDescriptor.fragmentFunction = pendingFragment;
    pendingDescriptor.colorAttachments[0].pixelFormat =
        MTLPixelFormatBGRA8Unorm;
    pendingDescriptor.colorAttachments[0].blendingEnabled = YES;
    pendingDescriptor.colorAttachments[0].sourceRGBBlendFactor =
        MTLBlendFactorOne;
    pendingDescriptor.colorAttachments[0].destinationRGBBlendFactor =
        MTLBlendFactorOneMinusSourceAlpha;
    pendingDescriptor.colorAttachments[0].sourceAlphaBlendFactor =
        MTLBlendFactorOne;
    pendingDescriptor.colorAttachments[0].destinationAlphaBlendFactor =
        MTLBlendFactorOneMinusSourceAlpha;
    _pendingPipeline =
        [_device newRenderPipelineStateWithDescriptor:pendingDescriptor
                                                error:&pipelineError];
    if (_pendingPipeline == nil || pipelineError != nil) {
      NSLog(@"pet pending pipeline failed: %@", pipelineError);
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

- (BOOL)preparePendingInstances:(uint32_t)pendingCount {
  if (_pendingInstanceCount == pendingCount) {
    return pendingCount == 0 || _pendingInstances != nil;
  }
  _pendingInstanceCount = pendingCount;
  _pendingInstances = nil;
  if (pendingCount == 0) {
    return YES;
  }

  std::vector<PetPendingInstance> instances;
  instances.reserve(pendingCount);
  const uint32_t ringCount = PetPendingRingCount(pendingCount);
  uint64_t ringStart = 0;
  uint32_t ringIndex = 0;
  uint64_t capacity = PetPendingRingCapacity(0);
  for (uint32_t index = 0; index < pendingCount; ++index) {
    while ((uint64_t)index >= ringStart + capacity) {
      ringStart += capacity;
      ++ringIndex;
      capacity = PetPendingRingCapacity(ringIndex);
    }
    const uint64_t remaining = (uint64_t)pendingCount - ringStart;
    const uint32_t dotsInRing =
        (uint32_t)std::min<uint64_t>(remaining, capacity);
    const uint32_t indexInRing = (uint32_t)((uint64_t)index - ringStart);
    const float radius =
        ringCount <= 1
            ? 0.78f
            : 0.62f + 0.28f * (float)ringIndex /
                          (float)std::max(1u, ringCount - 1);
    const float stagger = ringIndex % 2 == 0 ? 0.0f : 0.5f;
    const float angle =
        -1.5707963268f +
        6.2831853072f * ((float)indexInRing + stagger) /
            (float)std::max(1u, dotsInRing);
    const float arcDiameter =
        6.2831853072f * radius / (float)std::max(1u, dotsInRing) *
        0.55f;
    const float ringDiameter =
        ringCount <= 1
            ? 0.064f
            : 0.28f / (float)std::max(1u, ringCount - 1) * 0.62f;
    const float diameter =
        std::min(0.064f, std::min(arcDiameter, ringDiameter));
    instances.push_back({
        {cosf(angle) * radius, sinf(angle) * radius},
        diameter,
        0.0f,
    });
  }
  _pendingInstances =
      [_device newBufferWithBytes:instances.data()
                          length:instances.size() *
                                 sizeof(PetPendingInstance)
                         options:MTLResourceStorageModeShared];
  return _pendingInstances != nil;
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
          commandBuffer:(id<MTLCommandBuffer>)commandBuffer
                   stats:(PetRenderStats *)stats {
  if (output == nil || capture == nil || commandBuffer == nil) {
    return NO;
  }
  if (![self preparePendingInstances:uniforms.pending_count]) {
    return NO;
  }
  if (stats != nullptr) {
    *stats = {};
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
  if (stats != nullptr) {
    stats->base_draw_calls = 1;
  }
  if (uniforms.pending_count > 0) {
    [encoder setRenderPipelineState:_pendingPipeline];
    [encoder setVertexBuffer:_pendingInstances offset:0 atIndex:1];
    [encoder setVertexBytes:&uniforms
                     length:sizeof(uniforms)
                    atIndex:2];
    [encoder drawPrimitives:MTLPrimitiveTypeTriangle
                vertexStart:0
                vertexCount:6
              instanceCount:uniforms.pending_count];
    if (stats != nullptr) {
      stats->pending_draw_calls = 1;
      stats->pending_instances = uniforms.pending_count;
      stats->fragment_pending_iterations = 0;
    }
  }
  [encoder endEncoding];
  return YES;
}

- (uint32_t)drawSurface:(IOSurfaceRef)surface
               uniforms:(PetRenderUniforms)uniforms {
  CAMetalLayer *layer = _layer;
  if (layer == nil) {
    return PET_RENDER_DRAW_FATAL;
  }
  id<CAMetalDrawable> drawable = [layer nextDrawable];
  if (drawable == nil) {
    return PET_RENDER_DRAW_TRANSIENT;
  }
  id<MTLCommandBuffer> commandBuffer = [_commandQueue commandBuffer];
  if (![self encodeToTexture:drawable.texture
                     capture:[self textureForSurface:surface]
                    uniforms:uniforms
               commandBuffer:commandBuffer
                        stats:nullptr]) {
    return PET_RENDER_DRAW_FATAL;
  }
  [commandBuffer presentDrawable:drawable];
  [commandBuffer commit];
  return PET_RENDER_DRAW_OK;
}

- (BOOL)drawBytes:(const uint8_t *)bytes
            width:(uint32_t)width
           height:(uint32_t)height
         uniforms:(PetRenderUniforms)uniforms
           output:(uint8_t *)output
         capacity:(uint64_t)capacity
            stats:(PetRenderStats *)stats {
  const uint64_t required =
      (uint64_t)width * (uint64_t)height * (uint64_t)4;
  if (width == 0 || height == 0 || output == nullptr ||
      capacity < required || (uniforms.mode == 0 && bytes == nullptr)) {
    return NO;
  }
  id<MTLTexture> capture = _transparentTexture;
  if (bytes != nullptr) {
    std::vector<uint8_t> bgraInput(required);
    for (uint64_t offset = 0; offset < required; offset += 4) {
      bgraInput[offset] = bytes[offset + 2];
      bgraInput[offset + 1] = bytes[offset + 1];
      bgraInput[offset + 2] = bytes[offset];
      bgraInput[offset + 3] = bytes[offset + 3];
    }
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
                 withBytes:bgraInput.data()
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
  uniforms.viewport_px[0] = (float)width;
  uniforms.viewport_px[1] = (float)height;
  id<MTLCommandBuffer> commandBuffer = [_commandQueue commandBuffer];
  if (![self encodeToTexture:outputTexture
                     capture:capture
                    uniforms:uniforms
               commandBuffer:commandBuffer
                        stats:stats]) {
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
  for (uint64_t offset = 0; offset < required; offset += 4) {
    std::swap(output[offset], output[offset + 2]);
  }
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

extern "C" uint32_t mac_renderer_draw(void *handle, IOSurfaceRef surface,
                                      PetRenderUniforms uniforms) {
  if (handle == nullptr) {
    return PET_RENDER_DRAW_FATAL;
  }
  return [(__bridge BPPetMetalRenderer *)handle drawSurface:surface
                                                   uniforms:uniforms];
}

extern "C" uint64_t pet_test_render_rgba(
    const uint8_t *input, uint32_t width, uint32_t height,
    PetRenderUniforms uniforms, uint8_t *output, uint64_t output_capacity,
    PetRenderStats *stats, const char *metal_source) {
  void *handle = mac_renderer_create(metal_source, nullptr);
  if (handle == nullptr) {
    return 0;
  }
  const BOOL rendered =
      [(__bridge BPPetMetalRenderer *)handle drawBytes:input
                                                width:width
                                               height:height
                                             uniforms:uniforms
                                               output:output
                                             capacity:output_capacity
                                                stats:stats];
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
