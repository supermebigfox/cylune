#include "callback_guard.h"

#include <cassert>
#include <cstdint>

namespace {

bool callbackInvoked = false;

void ThrowingCallback(uint32_t, const char *, double, double, uint64_t) {
  callbackInvoked = true;
  throw 7;
}

} // namespace

int main() {
  bool continuedAfterCallback = false;
  InvokePetCallbackNoThrow(&ThrowingCallback, 4, nullptr, 0.0, 0.0, 0);
  continuedAfterCallback = true;

  assert(callbackInvoked);
  assert(continuedAfterCallback);
  InvokePetCallbackNoThrow(nullptr, 4, nullptr, 0.0, 0.0, 0);
}
