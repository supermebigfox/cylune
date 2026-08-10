#ifndef CYLUNE_WINDOWS_PET_WINDOW_H
#define CYLUNE_WINDOWS_PET_WINDOW_H

#include "bridge.h"

#include <memory>

class PetWindow {
 public:
  static std::unique_ptr<PetWindow> create(PetCallback callback);

  ~PetWindow();

  PetWindow(const PetWindow &) = delete;
  PetWindow &operator=(const PetWindow &) = delete;

  bool apply(PetConfig config);
  void show();
  void hide();
  void reset();
  void finishDrop(uint64_t generation, uint32_t result);
  uint32_t shutdown();

 private:
  struct Impl;

  explicit PetWindow(PetCallback callback);
  bool start();

  std::shared_ptr<Impl> impl_;
};

#endif
