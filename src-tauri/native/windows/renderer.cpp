#ifndef _WIN32_WINNT
#define _WIN32_WINNT 0x0A00
#endif
#ifndef NOMINMAX
#define NOMINMAX
#endif

#include "renderer.h"
#include "render_state.h"

#include <d3d11.h>
#include <d3dcompiler.h>
#include <dcomp.h>
#include <dxgi1_2.h>
#include <wrl/client.h>

#include <array>
#include <cstring>
#include <new>
#include <string>

using Microsoft::WRL::ComPtr;

namespace {

struct alignas(16) ShaderParams {
  float resolution[2];
  float time;
  float size;
  float brightness;
  float speed;
  uint32_t style;
  uint32_t pendingCount;
  float center[2];
  float ingestProgress;
  float ejectProgress;
  float pullGain;
  float successJetProgress;
  float padding1[2];
};

static_assert(sizeof(ShaderParams) == 64, "HLSL constant layout changed");

void LogRendererError(const char *message) noexcept {
  OutputDebugStringA("CYLUNE black-hole renderer: ");
  OutputDebugStringA(message == nullptr ? "unknown failure" : message);
  OutputDebugStringA("\n");
}

void LogShaderError(ID3DBlob *errors) noexcept {
  if (errors == nullptr || errors->GetBufferPointer() == nullptr ||
      errors->GetBufferSize() == 0) {
    LogRendererError("shader compilation failed");
    return;
  }
  try {
    std::string text(static_cast<const char *>(errors->GetBufferPointer()),
                     errors->GetBufferSize());
    text.push_back('\0');
    OutputDebugStringA("CYLUNE black-hole shader compile error: ");
    OutputDebugStringA(text.c_str());
    OutputDebugStringA("\n");
  } catch (...) {
    LogRendererError("shader compilation failed");
  }
}

bool CompileShader(const char *source, const char *entry, const char *target,
                   ComPtr<ID3DBlob> *bytecode) noexcept {
  if (source == nullptr || source[0] == '\0' || bytecode == nullptr) {
    return false;
  }
  ComPtr<ID3DBlob> errors;
  const HRESULT result = D3DCompile(
      source, std::strlen(source), "CYLUNE.BlackHole.hlsl", nullptr, nullptr,
      entry, target,
      D3DCOMPILE_ENABLE_STRICTNESS | D3DCOMPILE_OPTIMIZATION_LEVEL3, 0,
      bytecode->ReleaseAndGetAddressOf(), errors.GetAddressOf());
  if (FAILED(result)) {
    LogShaderError(errors.Get());
    return false;
  }
  return true;
}

}  // namespace

struct BlackHoleRenderer::Impl {
  HWND window = nullptr;
  bool ready = false;
  SurfacePrimeState surface;
  uint32_t width = 0;
  uint32_t height = 0;
  ComPtr<ID3D11Device> device;
  ComPtr<ID3D11DeviceContext> context;
  ComPtr<IDXGISwapChain1> swapChain;
  ComPtr<ID3D11RenderTargetView> renderTarget;
  ComPtr<ID3D11VertexShader> vertexShader;
  ComPtr<ID3D11PixelShader> pixelShader;
  ComPtr<ID3D11Buffer> constantBuffer;
  ComPtr<ID3D11SamplerState> sampler;
  ComPtr<ID3D11BlendState> blendState;
  ComPtr<ID3D11RasterizerState> rasterizerState;
  ComPtr<ID3D11Texture2D> fallbackTexture;
  ComPtr<ID3D11ShaderResourceView> fallbackView;
  ComPtr<IDCompositionDevice> compositionDevice;
  ComPtr<IDCompositionTarget> compositionTarget;
  ComPtr<IDCompositionVisual> compositionVisual;

  bool createRenderTarget() noexcept {
    renderTarget.Reset();
    ComPtr<ID3D11Texture2D> backBuffer;
    if (FAILED(swapChain->GetBuffer(
            0, IID_PPV_ARGS(backBuffer.ReleaseAndGetAddressOf())))) {
      return false;
    }
    return SUCCEEDED(device->CreateRenderTargetView(
        backBuffer.Get(), nullptr, renderTarget.ReleaseAndGetAddressOf()));
  }

  PresentDisposition primeSurface() noexcept {
    if (renderTarget == nullptr || swapChain == nullptr || context == nullptr) {
      return PresentDisposition::DeviceFailure;
    }
    surface.invalidatePrime();
    constexpr std::array<float, 4> transparent = {0.0f, 0.0f, 0.0f, 0.0f};
    context->ClearRenderTargetView(renderTarget.Get(), transparent.data());
    ID3D11RenderTargetView *target = renderTarget.Get();
    context->OMSetRenderTargets(1, &target, nullptr);
    ID3D11RenderTargetView *noTarget = nullptr;
    context->OMSetRenderTargets(1, &noTarget, nullptr);
    const HRESULT present = swapChain->Present(0, DXGI_PRESENT_DO_NOT_WAIT);
    context->ClearState();
    const PresentDisposition disposition = ClassifyPresentResult(
        static_cast<int32_t>(present),
        static_cast<int32_t>(DXGI_ERROR_WAS_STILL_DRAWING));
    surface.applyPrimePresent(disposition);
    return disposition;
  }

  bool initialize(HWND targetWindow, const char *source) noexcept {
    window = targetWindow;
    if (window == nullptr || source == nullptr) return false;

    UINT deviceFlags = D3D11_CREATE_DEVICE_BGRA_SUPPORT;
    if (FAILED(D3D11CreateDevice(
            nullptr, D3D_DRIVER_TYPE_HARDWARE, nullptr, deviceFlags, nullptr, 0,
            D3D11_SDK_VERSION, device.ReleaseAndGetAddressOf(), nullptr,
            context.ReleaseAndGetAddressOf()))) {
      LogRendererError("D3D11 device creation failed");
      return false;
    }

    ComPtr<IDXGIDevice> dxgiDevice;
    if (FAILED(device.As(&dxgiDevice))) return false;
    ComPtr<IDXGIAdapter> adapter;
    if (FAILED(dxgiDevice->GetAdapter(adapter.ReleaseAndGetAddressOf()))) {
      return false;
    }
    ComPtr<IDXGIFactory2> factory;
    if (FAILED(adapter->GetParent(
            IID_PPV_ARGS(factory.ReleaseAndGetAddressOf())))) {
      return false;
    }

    DXGI_SWAP_CHAIN_DESC1 swapDescription{};
    swapDescription.Width = 1;
    swapDescription.Height = 1;
    swapDescription.Format = DXGI_FORMAT_B8G8R8A8_UNORM;
    swapDescription.Stereo = FALSE;
    swapDescription.SampleDesc.Count = 1;
    swapDescription.BufferUsage = DXGI_USAGE_RENDER_TARGET_OUTPUT;
    swapDescription.BufferCount = 2;
    swapDescription.Scaling = DXGI_SCALING_STRETCH;
    swapDescription.SwapEffect = DXGI_SWAP_EFFECT_FLIP_SEQUENTIAL;
    swapDescription.AlphaMode = DXGI_ALPHA_MODE_PREMULTIPLIED;
    if (FAILED(factory->CreateSwapChainForComposition(
            device.Get(), &swapDescription, nullptr,
            swapChain.ReleaseAndGetAddressOf()))) {
      LogRendererError("composition swap chain creation failed");
      return false;
    }

    if (FAILED(DCompositionCreateDevice(
            dxgiDevice.Get(),
            IID_PPV_ARGS(compositionDevice.ReleaseAndGetAddressOf()))) ||
        FAILED(compositionDevice->CreateTargetForHwnd(
            window, TRUE, compositionTarget.ReleaseAndGetAddressOf())) ||
        FAILED(compositionDevice->CreateVisual(
            compositionVisual.ReleaseAndGetAddressOf())) ||
        FAILED(compositionVisual->SetContent(swapChain.Get())) ||
        FAILED(compositionTarget->SetRoot(compositionVisual.Get())) ||
        FAILED(compositionDevice->Commit())) {
      LogRendererError("DirectComposition visual creation failed");
      return false;
    }

    ComPtr<ID3DBlob> vertexBytecode;
    ComPtr<ID3DBlob> pixelBytecode;
    if (!CompileShader(source, "vs_main", "vs_5_0", &vertexBytecode) ||
        !CompileShader(source, "ps_main", "ps_5_0", &pixelBytecode)) {
      return false;
    }
    if (FAILED(device->CreateVertexShader(
            vertexBytecode->GetBufferPointer(), vertexBytecode->GetBufferSize(),
            nullptr, vertexShader.ReleaseAndGetAddressOf())) ||
        FAILED(device->CreatePixelShader(
            pixelBytecode->GetBufferPointer(), pixelBytecode->GetBufferSize(),
            nullptr, pixelShader.ReleaseAndGetAddressOf()))) {
      LogRendererError("shader object creation failed");
      return false;
    }

    D3D11_BUFFER_DESC bufferDescription{};
    bufferDescription.ByteWidth = sizeof(ShaderParams);
    bufferDescription.Usage = D3D11_USAGE_DEFAULT;
    bufferDescription.BindFlags = D3D11_BIND_CONSTANT_BUFFER;
    if (FAILED(device->CreateBuffer(&bufferDescription, nullptr,
                                    constantBuffer.ReleaseAndGetAddressOf()))) {
      return false;
    }

    D3D11_SAMPLER_DESC samplerDescription{};
    samplerDescription.Filter = D3D11_FILTER_MIN_MAG_MIP_LINEAR;
    samplerDescription.AddressU = D3D11_TEXTURE_ADDRESS_CLAMP;
    samplerDescription.AddressV = D3D11_TEXTURE_ADDRESS_CLAMP;
    samplerDescription.AddressW = D3D11_TEXTURE_ADDRESS_CLAMP;
    samplerDescription.MaxLOD = D3D11_FLOAT32_MAX;
    if (FAILED(device->CreateSamplerState(
            &samplerDescription, sampler.ReleaseAndGetAddressOf()))) {
      return false;
    }

    D3D11_BLEND_DESC blendDescription{};
    blendDescription.RenderTarget[0].BlendEnable = TRUE;
    blendDescription.RenderTarget[0].SrcBlend = D3D11_BLEND_SRC_ALPHA;
    blendDescription.RenderTarget[0].DestBlend = D3D11_BLEND_INV_SRC_ALPHA;
    blendDescription.RenderTarget[0].BlendOp = D3D11_BLEND_OP_ADD;
    blendDescription.RenderTarget[0].SrcBlendAlpha = D3D11_BLEND_ONE;
    blendDescription.RenderTarget[0].DestBlendAlpha =
        D3D11_BLEND_INV_SRC_ALPHA;
    blendDescription.RenderTarget[0].BlendOpAlpha = D3D11_BLEND_OP_ADD;
    blendDescription.RenderTarget[0].RenderTargetWriteMask =
        D3D11_COLOR_WRITE_ENABLE_ALL;
    if (FAILED(device->CreateBlendState(
            &blendDescription, blendState.ReleaseAndGetAddressOf()))) {
      return false;
    }

    D3D11_RASTERIZER_DESC rasterizerDescription{};
    rasterizerDescription.FillMode = D3D11_FILL_SOLID;
    rasterizerDescription.CullMode = D3D11_CULL_NONE;
    rasterizerDescription.DepthClipEnable = TRUE;
    if (FAILED(device->CreateRasterizerState(
            &rasterizerDescription,
            rasterizerState.ReleaseAndGetAddressOf()))) {
      return false;
    }

    const uint32_t transparentPixel = 0;
    D3D11_TEXTURE2D_DESC textureDescription{};
    textureDescription.Width = 1;
    textureDescription.Height = 1;
    textureDescription.MipLevels = 1;
    textureDescription.ArraySize = 1;
    textureDescription.Format = DXGI_FORMAT_B8G8R8A8_UNORM;
    textureDescription.SampleDesc.Count = 1;
    textureDescription.Usage = D3D11_USAGE_IMMUTABLE;
    textureDescription.BindFlags = D3D11_BIND_SHADER_RESOURCE;
    D3D11_SUBRESOURCE_DATA textureData{};
    textureData.pSysMem = &transparentPixel;
    textureData.SysMemPitch = sizeof(transparentPixel);
    if (FAILED(device->CreateTexture2D(
            &textureDescription, &textureData,
            fallbackTexture.ReleaseAndGetAddressOf())) ||
        FAILED(device->CreateShaderResourceView(
            fallbackTexture.Get(), nullptr,
            fallbackView.ReleaseAndGetAddressOf())) ||
        !createRenderTarget()) {
      return false;
    }

    width = 1;
    height = 1;
    if (primeSurface() == PresentDisposition::DeviceFailure) {
      LogRendererError("transparent surface prime failed");
      return false;
    }
    ready = true;
    return true;
  }

  void detachComposition() noexcept {
    if (compositionVisual != nullptr) {
      (void)compositionVisual->SetContent(nullptr);
    }
    if (compositionTarget != nullptr) {
      (void)compositionTarget->SetRoot(nullptr);
    }
    if (compositionDevice != nullptr) (void)compositionDevice->Commit();
  }

  void fail(const char *message) noexcept {
    LogRendererError(message);
    ready = false;
    surface.invalidatePrime();
    if (context != nullptr) context->ClearState();
    detachComposition();
  }

  void releaseAll() noexcept {
    ready = false;
    surface.invalidatePrime();
    if (context != nullptr) context->ClearState();
    detachComposition();
    compositionVisual.Reset();
    compositionTarget.Reset();
    compositionDevice.Reset();
    fallbackView.Reset();
    fallbackTexture.Reset();
    rasterizerState.Reset();
    blendState.Reset();
    sampler.Reset();
    constantBuffer.Reset();
    pixelShader.Reset();
    vertexShader.Reset();
    renderTarget.Reset();
    swapChain.Reset();
    context.Reset();
    device.Reset();
    width = 0;
    height = 0;
    window = nullptr;
  }
};

BlackHoleRenderer::BlackHoleRenderer() : impl_(new (std::nothrow) Impl) {}

BlackHoleRenderer::~BlackHoleRenderer() { shutdown(); }

std::unique_ptr<BlackHoleRenderer> BlackHoleRenderer::create(
    HWND window, const char *hlslSource) {
  std::unique_ptr<BlackHoleRenderer> renderer(
      new (std::nothrow) BlackHoleRenderer());
  if (renderer == nullptr || renderer->impl_ == nullptr) return nullptr;
  if (!renderer->impl_->initialize(window, hlslSource)) {
    renderer->impl_->releaseAll();
  }
  return renderer;
}

bool BlackHoleRenderer::resize(uint32_t pixelWidth,
                               uint32_t pixelHeight) noexcept {
  if (impl_ == nullptr || !impl_->ready || pixelWidth == 0 ||
      pixelHeight == 0) {
    return false;
  }
  if (impl_->width == pixelWidth && impl_->height == pixelHeight) return true;
  impl_->surface.invalidatePrime();
  impl_->context->ClearState();
  impl_->renderTarget.Reset();
  const HRESULT result = impl_->swapChain->ResizeBuffers(
      2, pixelWidth, pixelHeight, DXGI_FORMAT_B8G8R8A8_UNORM, 0);
  if (FAILED(result) || !impl_->createRenderTarget()) {
    impl_->fail("swap chain resize failed");
    return false;
  }
  impl_->width = pixelWidth;
  impl_->height = pixelHeight;
  const PresentDisposition disposition = impl_->primeSurface();
  if (disposition == PresentDisposition::DeviceFailure) {
    impl_->fail("transparent surface prime after resize failed");
    return false;
  }
  return true;
}

bool BlackHoleRenderer::prime() noexcept {
  if (impl_ == nullptr || !impl_->ready) return false;
  const PresentDisposition disposition = impl_->primeSurface();
  if (disposition == PresentDisposition::Presented) return true;
  if (disposition == PresentDisposition::Retry) return false;
  impl_->fail("transparent surface prime failed");
  return false;
}

bool BlackHoleRenderer::render(const RendererFrame &frame) noexcept {
  if (impl_ == nullptr || !impl_->ready || !impl_->surface.canRender() ||
      impl_->renderTarget == nullptr || impl_->width == 0 || impl_->height == 0) {
    return false;
  }

  ShaderParams params{};
  params.resolution[0] = static_cast<float>(impl_->width);
  params.resolution[1] = static_cast<float>(impl_->height);
  params.time = static_cast<float>(frame.animationTime);
  params.size = static_cast<float>(frame.visualDiameterPixels /
                                   (1.05 * impl_->height));
  params.brightness = frame.brightness;
  params.speed = 1.0f;
  params.style = frame.shaderStyle;
  params.pendingCount = frame.pendingCount;
  params.center[0] = frame.centerX;
  params.center[1] = frame.centerY;
  params.ingestProgress = frame.ingestProgress;
  params.ejectProgress = frame.ejectProgress;
  params.pullGain = frame.pullGain;
  params.successJetProgress = frame.successJetProgress;
  impl_->context->UpdateSubresource(impl_->constantBuffer.Get(), 0, nullptr,
                                    &params, 0, 0);

  constexpr std::array<float, 4> transparent = {0.0f, 0.0f, 0.0f, 0.0f};
  impl_->context->ClearRenderTargetView(impl_->renderTarget.Get(),
                                        transparent.data());
  ID3D11RenderTargetView *target = impl_->renderTarget.Get();
  impl_->context->OMSetRenderTargets(1, &target, nullptr);
  impl_->context->OMSetBlendState(impl_->blendState.Get(), nullptr,
                                  0xffffffffu);
  D3D11_VIEWPORT viewport{};
  viewport.Width = static_cast<float>(impl_->width);
  viewport.Height = static_cast<float>(impl_->height);
  viewport.MaxDepth = 1.0f;
  impl_->context->RSSetViewports(1, &viewport);
  impl_->context->RSSetState(impl_->rasterizerState.Get());
  impl_->context->IASetInputLayout(nullptr);
  impl_->context->IASetPrimitiveTopology(
      D3D11_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
  impl_->context->VSSetShader(impl_->vertexShader.Get(), nullptr, 0);
  impl_->context->PSSetShader(impl_->pixelShader.Get(), nullptr, 0);
  ID3D11Buffer *constant = impl_->constantBuffer.Get();
  impl_->context->PSSetConstantBuffers(0, 1, &constant);
  ID3D11SamplerState *sampler = impl_->sampler.Get();
  impl_->context->PSSetSamplers(0, 1, &sampler);
  ID3D11ShaderResourceView *desktop =
      frame.desktop == nullptr ? impl_->fallbackView.Get() : frame.desktop;
  impl_->context->PSSetShaderResources(0, 1, &desktop);
  impl_->context->Draw(3, 0);

  ID3D11ShaderResourceView *noResource = nullptr;
  impl_->context->PSSetShaderResources(0, 1, &noResource);
  ID3D11RenderTargetView *noTarget = nullptr;
  impl_->context->OMSetRenderTargets(1, &noTarget, nullptr);
  const HRESULT present = impl_->swapChain->Present(0, DXGI_PRESENT_DO_NOT_WAIT);
  impl_->context->ClearState();
  const PresentDisposition disposition = ClassifyPresentResult(
      static_cast<int32_t>(present),
      static_cast<int32_t>(DXGI_ERROR_WAS_STILL_DRAWING));
  if (disposition == PresentDisposition::Retry) return true;
  if (disposition == PresentDisposition::DeviceFailure) {
    impl_->fail("swap chain present failed");
    return false;
  }
  return true;
}

void BlackHoleRenderer::setVisible(bool visible) noexcept {
  if (impl_ == nullptr || !impl_->ready) return;
  if (visible) {
    (void)impl_->surface.reveal();
  } else {
    impl_->surface.conceal();
  }
}

void BlackHoleRenderer::shutdown() noexcept {
  if (impl_ != nullptr) impl_->releaseAll();
}

bool BlackHoleRenderer::available() const noexcept {
  return impl_ != nullptr && impl_->ready;
}

bool BlackHoleRenderer::primed() const noexcept {
  return impl_ != nullptr && impl_->surface.primed();
}
