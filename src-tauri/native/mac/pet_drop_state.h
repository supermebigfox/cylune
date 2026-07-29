#ifndef CYLUNE_PET_DROP_STATE_H
#define CYLUNE_PET_DROP_STATE_H

#include "bridge.h"

#include <cstdint>
#include <string>

class PetDropSession {
 public:
  uint64_t enter(const char *path, uint32_t fileKind) {
    if (waitingForAck_) return 0;
    cancelHover();
    if (path == nullptr || path[0] == '\0' ||
        (fileKind != PET_FILE_3MF && fileKind != PET_FILE_GCODE &&
         fileKind != PET_FILE_OTHER)) {
      return 0;
    }
    nextGeneration_ += 1;
    if (nextGeneration_ == 0) nextGeneration_ += 1;
    generation_ = nextGeneration_;
    path_ = path;
    fileKind_ = fileKind;
    hovering_ = true;
    return generation_;
  }

  bool submit(uint64_t generation, const char *path) {
    if (!hovering_ || waitingForAck_ || generation == 0 ||
        generation != generation_ || path == nullptr || path_ != path) {
      return false;
    }
    hovering_ = false;
    waitingForAck_ = true;
    return true;
  }

  bool finish(uint64_t generation, uint32_t result) {
    if (!waitingForAck_ || generation == 0 || generation != generation_ ||
        (result != PET_DROP_ACCEPTED && result != PET_DROP_REJECTED)) {
      return false;
    }
    waitingForAck_ = false;
    generation_ = 0;
    path_.clear();
    fileKind_ = PET_FILE_NONE;
    return true;
  }

  void cancelHover() {
    if (waitingForAck_) return;
    hovering_ = false;
    generation_ = 0;
    path_.clear();
    fileKind_ = PET_FILE_NONE;
  }

  uint64_t generation() const { return generation_; }
  uint32_t fileKind() const { return fileKind_; }
  bool waitingForAck() const { return waitingForAck_; }

 private:
  uint64_t nextGeneration_ = 0;
  uint64_t generation_ = 0;
  std::string path_;
  uint32_t fileKind_ = PET_FILE_NONE;
  bool hovering_ = false;
  bool waitingForAck_ = false;
};

#endif
