#ifndef BAMBU_POOLS_PET_LIFECYCLE_H
#define BAMBU_POOLS_PET_LIFECYCLE_H

class PetMonitorLifecycle {
 public:
  bool show() {
    if (destroyed_ || monitor_active_) {
      return false;
    }
    monitor_active_ = true;
    return true;
  }

  bool hide() {
    if (!monitor_active_) {
      return false;
    }
    monitor_active_ = false;
    return true;
  }

  bool destroy() {
    if (destroyed_) {
      return false;
    }
    monitor_active_ = false;
    destroyed_ = true;
    return true;
  }

  bool monitor_active() const { return monitor_active_; }
  bool destroyed() const { return destroyed_; }

 private:
  bool monitor_active_ = false;
  bool destroyed_ = false;
};

#endif
