#ifndef _WIN32_WINNT
#define _WIN32_WINNT 0x0A00
#endif
#ifndef NOMINMAX
#define NOMINMAX
#endif
#ifndef UNICODE
#define UNICODE
#endif
#ifndef _UNICODE
#define _UNICODE
#endif

#include "drop_target.h"

#include <windows.h>
#include <objidl.h>
#include <ShlObj_core.h>

#include <atomic>
#include <cassert>
#include <cstdint>
#include <cstring>
#include <stdexcept>
#include <string>
#include <utility>

namespace {

struct CallbackRecord {
  uint32_t entered = 0;
  uint32_t exited = 0;
  uint32_t dropped = 0;
  uint64_t generation = 0;
  bool throwAfterRecord = false;
};

CallbackRecord *activeRecord = nullptr;

void RecordCallback(uint32_t kind, const char *, double, double,
                    uint64_t eventValue) {
  assert(activeRecord != nullptr);
  if (kind == 3) ++activeRecord->entered;
  if (kind == 4) ++activeRecord->exited;
  if (kind == 5) {
    ++activeRecord->dropped;
    activeRecord->generation = eventValue;
  }
  if (activeRecord->throwAfterRecord) throw std::runtime_error("callback");
}

void RecordVisual(void *context, PetDropVisualState state) {
  *static_cast<PetDropVisualState *>(context) = state;
}

class DropDataObject final : public IDataObject {
 public:
  explicit DropDataObject(std::wstring path) : path_(std::move(path)) {}

  HRESULT STDMETHODCALLTYPE QueryInterface(REFIID interfaceId,
                                           void **object) override {
    if (object == nullptr) return E_POINTER;
    *object = nullptr;
    if (IsEqualIID(interfaceId, IID_IUnknown) ||
        IsEqualIID(interfaceId, IID_IDataObject)) {
      *object = static_cast<IDataObject *>(this);
      AddRef();
      return S_OK;
    }
    return E_NOINTERFACE;
  }

  ULONG STDMETHODCALLTYPE AddRef() override { return ++references_; }

  ULONG STDMETHODCALLTYPE Release() override { return --references_; }

  HRESULT STDMETHODCALLTYPE GetData(FORMATETC *format,
                                    STGMEDIUM *medium) override {
    if (QueryGetData(format) != S_OK || medium == nullptr) return E_INVALIDARG;
    const SIZE_T byteCount = sizeof(DROPFILES) +
                             (path_.size() + 2) * sizeof(wchar_t);
    HGLOBAL memory = GlobalAlloc(GMEM_MOVEABLE | GMEM_ZEROINIT, byteCount);
    if (memory == nullptr) return E_OUTOFMEMORY;
    void *locked = GlobalLock(memory);
    if (locked == nullptr) {
      (void)GlobalFree(memory);
      return E_OUTOFMEMORY;
    }
    auto *files = static_cast<DROPFILES *>(locked);
    files->pFiles = sizeof(DROPFILES);
    files->fWide = TRUE;
    auto *destination = reinterpret_cast<wchar_t *>(
        static_cast<unsigned char *>(locked) + sizeof(DROPFILES));
    std::memcpy(destination, path_.c_str(),
                (path_.size() + 1) * sizeof(wchar_t));
    (void)GlobalUnlock(memory);
    medium->tymed = TYMED_HGLOBAL;
    medium->hGlobal = memory;
    medium->pUnkForRelease = nullptr;
    return S_OK;
  }

  HRESULT STDMETHODCALLTYPE GetDataHere(FORMATETC *, STGMEDIUM *) override {
    return E_NOTIMPL;
  }

  HRESULT STDMETHODCALLTYPE QueryGetData(FORMATETC *format) override {
    return format != nullptr && format->cfFormat == CF_HDROP &&
                   format->dwAspect == DVASPECT_CONTENT &&
                   (format->tymed & TYMED_HGLOBAL) != 0
               ? S_OK
               : DV_E_FORMATETC;
  }

  HRESULT STDMETHODCALLTYPE GetCanonicalFormatEtc(FORMATETC *,
                                                   FORMATETC *output) override {
    if (output != nullptr) output->ptd = nullptr;
    return E_NOTIMPL;
  }

  HRESULT STDMETHODCALLTYPE SetData(FORMATETC *, STGMEDIUM *, BOOL) override {
    return E_NOTIMPL;
  }

  HRESULT STDMETHODCALLTYPE EnumFormatEtc(DWORD,
                                          IEnumFORMATETC **) override {
    return E_NOTIMPL;
  }

  HRESULT STDMETHODCALLTYPE DAdvise(FORMATETC *, DWORD, IAdviseSink *,
                                    DWORD *) override {
    return OLE_E_ADVISENOTSUPPORTED;
  }

  HRESULT STDMETHODCALLTYPE DUnadvise(DWORD) override {
    return OLE_E_ADVISENOTSUPPORTED;
  }

  HRESULT STDMETHODCALLTYPE EnumDAdvise(IEnumSTATDATA **) override {
    return OLE_E_ADVISENOTSUPPORTED;
  }

 private:
  ULONG references_ = 1;
  std::wstring path_;
};

struct TemporaryFile {
  TemporaryFile() {
    wchar_t directory[MAX_PATH]{};
    assert(GetTempPathW(MAX_PATH, directory) != 0);
    wchar_t name[MAX_PATH]{};
    assert(GetTempFileNameW(directory, L"cyl", 0, name) != 0);
    path = name;
  }

  ~TemporaryFile() { (void)DeleteFileW(path.c_str()); }

  std::wstring path;
};

POINTL ClientCenter(HWND window) {
  POINT center{300, 300};
  assert(ClientToScreen(window, &center));
  return {center.x, center.y};
}

PetDropTarget *NewTarget(HWND window, CallbackRecord *record,
                         PetDropVisualState *visual,
                         std::atomic<bool> *stopping) {
  activeRecord = record;
  PetDropTarget *target = PetDropTarget::create(
      window, &RecordCallback, &RecordVisual, visual, stopping);
  assert(target != nullptr);
  return target;
}

} // namespace

int main() {
  const HRESULT apartment = OleInitialize(nullptr);
  assert(SUCCEEDED(apartment));
  HWND window = CreateWindowExW(0, L"STATIC", L"", WS_POPUP, 100, 100, 600,
                                600, nullptr, nullptr,
                                GetModuleHandleW(nullptr), nullptr);
  assert(window != nullptr);
  TemporaryFile file;
  DropDataObject data(file.path);
  const POINTL center = ClientCenter(window);

  for (const DWORD disallowed : {DROPEFFECT_NONE, DROPEFFECT_MOVE,
                                 DROPEFFECT_LINK}) {
    CallbackRecord record;
    PetDropVisualState visual = PetDropVisualState::Idle;
    std::atomic<bool> stopping{false};
    PetDropTarget *target = NewTarget(window, &record, &visual, &stopping);
    DWORD effect = disallowed;
    assert(target->DragEnter(&data, 0, center, &effect) == S_OK);
    assert(effect == DROPEFFECT_NONE);
    effect = disallowed;
    assert(target->Drop(&data, 0, center, &effect) == S_OK);
    assert(effect == DROPEFFECT_NONE);
    assert(record.entered == 0);
    assert(record.dropped == 0);
    assert(visual == PetDropVisualState::Idle);
    target->deactivate();
    (void)target->Release();
  }

  for (const DWORD allowed : {DROPEFFECT_COPY,
                              DROPEFFECT_COPY | DROPEFFECT_MOVE}) {
    CallbackRecord record;
    PetDropVisualState visual = PetDropVisualState::Idle;
    std::atomic<bool> stopping{false};
    PetDropTarget *target = NewTarget(window, &record, &visual, &stopping);
    DWORD effect = allowed;
    assert(target->DragEnter(&data, 0, center, &effect) == S_OK);
    assert(effect == DROPEFFECT_COPY);
    assert(record.entered == 1);
    assert(visual == PetDropVisualState::Hover);
    assert(target->DragLeave() == S_OK);
    assert(record.exited == 1);
    assert(visual == PetDropVisualState::Idle);
    target->deactivate();
    (void)target->Release();
  }

  {
    CallbackRecord record;
    PetDropVisualState visual = PetDropVisualState::Idle;
    std::atomic<bool> stopping{false};
    PetDropTarget *target = NewTarget(window, &record, &visual, &stopping);
    DWORD effect = DROPEFFECT_COPY;
    assert(target->DragEnter(&data, 0, center, &effect) == S_OK);
    effect = DROPEFFECT_MOVE;
    assert(target->DragOver(0, center, &effect) == S_OK);
    assert(effect == DROPEFFECT_NONE);
    assert(record.exited == 1);
    assert(visual == PetDropVisualState::Idle);
    effect = DROPEFFECT_MOVE;
    assert(target->Drop(&data, 0, center, &effect) == S_OK);
    assert(effect == DROPEFFECT_NONE);
    assert(record.dropped == 0);
    target->deactivate();
    (void)target->Release();
  }

  {
    CallbackRecord record;
    record.throwAfterRecord = true;
    PetDropVisualState visual = PetDropVisualState::Idle;
    std::atomic<bool> stopping{false};
    PetDropTarget *target = NewTarget(window, &record, &visual, &stopping);
    DWORD effect = DROPEFFECT_COPY;
    assert(target->DragEnter(&data, 0, center, &effect) == S_OK);
    assert(effect == DROPEFFECT_COPY);
    assert(visual == PetDropVisualState::Hover);
    assert(target->DragLeave() == S_OK);
    assert(visual == PetDropVisualState::Idle);
    effect = DROPEFFECT_COPY;
    assert(target->DragEnter(&data, 0, center, &effect) == S_OK);
    assert(target->Drop(&data, 0, center, &effect) == S_OK);
    assert(record.generation != 0);
    assert(target->finish(record.generation, PET_DROP_ACCEPTED));
    target->deactivate();
    (void)target->Release();
  }

  activeRecord = nullptr;
  assert(DestroyWindow(window));
  OleUninitialize();
}
