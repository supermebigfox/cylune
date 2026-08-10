#ifndef CYLUNE_WINDOWS_PET_DROP_TARGET_H
#define CYLUNE_WINDOWS_PET_DROP_TARGET_H

#ifndef NOMINMAX
#define NOMINMAX
#endif
#ifndef UNICODE
#define UNICODE
#endif
#ifndef _UNICODE
#define _UNICODE
#endif

#include "bridge.h"
#include "drop_state.h"

#include <windows.h>
#include <oleidl.h>

#include <atomic>
#include <cstdint>

using PetDropVisualCallback = void (*)(void *context,
                                       PetDropVisualState state);

class PetDropTarget final : public IDropTarget {
 public:
  static PetDropTarget *create(HWND window, PetCallback callback,
                               PetDropVisualCallback visualCallback,
                               void *visualContext,
                               const std::atomic<bool> *stopping);

  HRESULT STDMETHODCALLTYPE QueryInterface(REFIID interfaceId,
                                           void **object) override;
  ULONG STDMETHODCALLTYPE AddRef() override;
  ULONG STDMETHODCALLTYPE Release() override;
  HRESULT STDMETHODCALLTYPE DragEnter(IDataObject *dataObject, DWORD keyState,
                                      POINTL point, DWORD *effect) override;
  HRESULT STDMETHODCALLTYPE DragOver(DWORD keyState, POINTL point,
                                     DWORD *effect) override;
  HRESULT STDMETHODCALLTYPE DragLeave() override;
  HRESULT STDMETHODCALLTYPE Drop(IDataObject *dataObject, DWORD keyState,
                                 POINTL point, DWORD *effect) override;

  bool finish(uint64_t generation, uint32_t result);
  void cancelHover();
  void deactivate();

 private:
  struct Impl;

  PetDropTarget(HWND window, PetCallback callback,
                PetDropVisualCallback visualCallback, void *visualContext,
                const std::atomic<bool> *stopping);
  ~PetDropTarget();

  std::atomic<ULONG> references_{1};
  Impl *impl_;
};

#endif
