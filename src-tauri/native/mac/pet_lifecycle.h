#ifndef CYLUNE_PET_LIFECYCLE_H
#define CYLUNE_PET_LIFECYCLE_H

#include <atomic>
#include <cstdint>

class PetApplyGenerationGate {
 public:
  uint64_t issue() {
    return issued_.fetch_add(1, std::memory_order_acq_rel) + 1;
  }

  bool accept(uint64_t generation) {
    uint64_t accepted = accepted_.load(std::memory_order_acquire);
    while (generation > accepted) {
      if (accepted_.compare_exchange_weak(
              accepted, generation, std::memory_order_acq_rel,
              std::memory_order_acquire)) {
        return true;
      }
    }
    return false;
  }

 private:
  std::atomic<uint64_t> issued_{0};
  std::atomic<uint64_t> accepted_{0};
};

class PetWindowLifecycle {
 public:
  void show() {
    if (!destroyed_) visible_ = true;
  }
  void hide() { visible_ = false; }
  void sleep() { sleeping_ = true; }
  void wake() { sleeping_ = false; }
  void destroy() {
    destroyed_ = true;
    visible_ = false;
    sleeping_ = false;
  }
  bool visible() const { return visible_; }
  bool sleeping() const { return sleeping_; }
  bool destroyed() const { return destroyed_; }

 private:
  bool visible_ = false;
  bool sleeping_ = false;
  bool destroyed_ = false;
};

#endif
