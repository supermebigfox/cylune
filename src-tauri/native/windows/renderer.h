#ifndef CYLUNE_WINDOWS_BLACK_HOLE_RENDERER_H
#define CYLUNE_WINDOWS_BLACK_HOLE_RENDERER_H

#ifndef NOMINMAX
#define NOMINMAX
#endif

#include <windows.h>

#include <cstdint>
#include <memory>

struct ID3D11ShaderResourceView;

struct RendererFrame {
  double animationTime = 0.0;
  double visualDiameterPixels = 300.0;
  float brightness = 1.0f;
  float rotationRate = 1.0f;
  uint32_t shaderStyle = 1;
  float centerX = 0.5f;
  float centerY = 0.5f;
  float ingestProgress = 0.0f;
  float ejectProgress = 0.0f;
  float pullGain = 1.0f;
  float successJetProgress = 0.0f;
  uint32_t pendingCount = 0;
  ID3D11ShaderResourceView *desktop = nullptr;
};

class BlackHoleRenderer {
 public:
  static std::unique_ptr<BlackHoleRenderer> create(HWND window,
                                                    const char *hlslSource);

  ~BlackHoleRenderer();

  BlackHoleRenderer(const BlackHoleRenderer &) = delete;
  BlackHoleRenderer &operator=(const BlackHoleRenderer &) = delete;

  bool resize(uint32_t pixelWidth, uint32_t pixelHeight) noexcept;
  bool render(const RendererFrame &frame) noexcept;
  void setVisible(bool visible) noexcept;
  void shutdown() noexcept;
  bool available() const noexcept;

 private:
  struct Impl;

  BlackHoleRenderer();
  std::unique_ptr<Impl> impl_;
};

#endif
