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

#include "drop_state.h"

#include <shellapi.h>

#include <algorithm>
#include <climits>
#include <cwctype>
#include <new>
#include <string>
#include <utility>
#include <vector>

namespace {

constexpr uint32_t kCallbackDropEntered = 3;
constexpr uint32_t kCallbackDropExited = 4;
constexpr uint32_t kCallbackFileDropped = 5;

struct StgMediumGuard {
  explicit StgMediumGuard(STGMEDIUM *value) : medium(value) {}
  ~StgMediumGuard() { ReleaseStgMedium(medium); }

  StgMediumGuard(const StgMediumGuard &) = delete;
  StgMediumGuard &operator=(const StgMediumGuard &) = delete;

  STGMEDIUM *medium;
};

bool IsSlash(wchar_t value) { return value == L'\\' || value == L'/'; }

bool IsAbsolutePath(const std::wstring &path) {
  if (path.size() >= 3 && std::iswalpha(path[0]) != 0 && path[1] == L':' &&
      IsSlash(path[2])) {
    return true;
  }
  if (path.size() < 5 || !IsSlash(path[0]) || !IsSlash(path[1])) {
    return false;
  }
  const size_t serverEnd = path.find_first_of(L"\\/", 2);
  return serverEnd != std::wstring::npos && serverEnd > 2 &&
         serverEnd + 1 < path.size();
}

bool IsOrdinaryFile(const std::wstring &path) {
  if (!IsAbsolutePath(path)) return false;
  const DWORD attributes = GetFileAttributesW(path.c_str());
  return attributes != INVALID_FILE_ATTRIBUTES &&
         (attributes & (FILE_ATTRIBUTE_DIRECTORY | FILE_ATTRIBUTE_DEVICE |
                        FILE_ATTRIBUTE_REPARSE_POINT)) == 0;
}

FileKind ClassifyFile(const std::wstring &path) {
  std::wstring lower(path);
  std::transform(lower.begin(), lower.end(), lower.begin(),
                 [](wchar_t value) {
                   return static_cast<wchar_t>(std::towlower(value));
                 });
  if (lower.size() >= 4 && lower.compare(lower.size() - 4, 4, L".3mf") == 0) {
    return FileKind::ThreeMf;
  }
  if (lower.size() >= 6 &&
      lower.compare(lower.size() - 6, 6, L".gcode") == 0) {
    return FileKind::GCode;
  }
  return FileKind::Other;
}

bool Utf16ToUtf8(const std::wstring &source, std::string *destination) {
  if (destination == nullptr || source.empty() ||
      source.size() > static_cast<size_t>(INT_MAX)) {
    return false;
  }
  const int sourceLength = static_cast<int>(source.size());
  const int required = WideCharToMultiByte(
      CP_UTF8, WC_ERR_INVALID_CHARS, source.data(), sourceLength, nullptr, 0,
      nullptr, nullptr);
  if (required <= 0) return false;
  std::string converted(static_cast<size_t>(required), '\0');
  const int written = WideCharToMultiByte(
      CP_UTF8, WC_ERR_INVALID_CHARS, source.data(), sourceLength,
      converted.data(), required, nullptr, nullptr);
  if (written != required) return false;
  *destination = std::move(converted);
  return true;
}

bool ExtractFirstOrdinaryFile(IDataObject *dataObject, std::wstring *path) {
  if (dataObject == nullptr || path == nullptr) return false;
  FORMATETC format{CF_HDROP, nullptr, DVASPECT_CONTENT, -1, TYMED_HGLOBAL};
  if (dataObject->QueryGetData(&format) != S_OK) return false;

  STGMEDIUM medium{};
  if (FAILED(dataObject->GetData(&format, &medium))) return false;
  const StgMediumGuard releaseMedium(&medium);
  bool extracted = false;
  if (medium.tymed == TYMED_HGLOBAL && medium.hGlobal != nullptr) {
    const HDROP drop = static_cast<HDROP>(medium.hGlobal);
    const UINT count = DragQueryFileW(drop, 0xFFFFFFFF, nullptr, 0);
    if (count > 0) {
      const UINT length = DragQueryFileW(drop, 0, nullptr, 0);
      if (length > 0) {
        std::vector<wchar_t> buffer(static_cast<size_t>(length) + 1, L'\0');
        if (DragQueryFileW(drop, 0, buffer.data(), length + 1) == length) {
          std::wstring candidate(buffer.data(), length);
          if (IsOrdinaryFile(candidate)) {
            *path = std::move(candidate);
            extracted = true;
          }
        }
      }
    }
  }
  return extracted;
}

} // namespace

struct PetDropTarget::Impl {
  Impl(HWND windowValue, PetCallback callbackValue,
       PetDropVisualCallback visualCallbackValue, void *visualContextValue,
       const std::atomic<bool> *stoppingValue)
      : window(windowValue),
        callback(callbackValue),
        visualCallback(visualCallbackValue),
        visualContext(visualContextValue),
        stopping(stoppingValue) {}

  HWND window;
  PetCallback callback;
  PetDropVisualCallback visualCallback;
  void *visualContext;
  const std::atomic<bool> *stopping;
  bool active = true;
  DropSession session;
  std::wstring candidatePath;
  FileKind candidateKind = FileKind::None;

  bool acceptingCallbacks() const {
    return active && stopping != nullptr &&
           !stopping->load(std::memory_order_acquire);
  }

  void setEffect(DWORD *effect, bool accepted) const {
    if (effect != nullptr) *effect = accepted ? DROPEFFECT_COPY : DROPEFFECT_NONE;
  }

  void setVisual(PetDropVisualState state) const {
    if (acceptingCallbacks() && visualCallback != nullptr) {
      visualCallback(visualContext, state);
    }
  }

  void emit(uint32_t kind, const char *payload = nullptr,
            uint64_t generation = 0) const {
    if (acceptingCallbacks() && callback != nullptr) {
      callback(kind, payload, 0.0, 0.0, generation);
    }
  }

  bool pointerInside(POINTL screenPoint) const {
    POINT clientPoint{screenPoint.x, screenPoint.y};
    RECT client{};
    if (!ScreenToClient(window, &clientPoint) ||
        !GetClientRect(window, &client)) {
      return false;
    }
    const double side = static_cast<double>(
        std::min(client.right - client.left, client.bottom - client.top));
    return PointerInsideDropTarget(static_cast<double>(clientPoint.x),
                                   static_cast<double>(clientPoint.y), side);
  }

  void exitHover() {
    if (session.leave()) {
      emit(kCallbackDropExited);
      setVisual(PetDropVisualState::Idle);
    }
  }

  bool enterHover() {
    if (candidatePath.empty() || session.waitingForAck()) return false;
    if (session.hovering()) return true;
    const uint64_t generation = session.enter(candidatePath, candidateKind);
    if (generation == 0) return false;
    emit(kCallbackDropEntered, nullptr, generation);
    setVisual(PetDropVisualState::Hover);
    return true;
  }

  bool updateHover(POINTL point) {
    if (!acceptingCallbacks() || candidatePath.empty() ||
        !pointerInside(point)) {
      exitHover();
      return false;
    }
    return enterHover();
  }

  void clearCandidate() {
    candidatePath.clear();
    candidateKind = FileKind::None;
  }
};

PetDropTarget::PetDropTarget(HWND window, PetCallback callback,
                             PetDropVisualCallback visualCallback,
                             void *visualContext,
                             const std::atomic<bool> *stopping)
    : impl_(new (std::nothrow)
                Impl(window, callback, visualCallback, visualContext,
                     stopping)) {}

PetDropTarget::~PetDropTarget() { delete impl_; }

PetDropTarget *PetDropTarget::create(HWND window, PetCallback callback,
                                     PetDropVisualCallback visualCallback,
                                     void *visualContext,
                                     const std::atomic<bool> *stopping) {
  if (window == nullptr || callback == nullptr || stopping == nullptr) {
    return nullptr;
  }
  PetDropTarget *target =
      new (std::nothrow) PetDropTarget(window, callback, visualCallback,
                                      visualContext, stopping);
  if (target == nullptr || target->impl_ == nullptr) {
    delete target;
    return nullptr;
  }
  return target;
}

HRESULT STDMETHODCALLTYPE PetDropTarget::QueryInterface(REFIID interfaceId,
                                                        void **object) {
  if (object == nullptr) return E_POINTER;
  *object = nullptr;
  if (IsEqualIID(interfaceId, IID_IUnknown) ||
      IsEqualIID(interfaceId, IID_IDropTarget)) {
    *object = static_cast<IDropTarget *>(this);
    AddRef();
    return S_OK;
  }
  return E_NOINTERFACE;
}

ULONG STDMETHODCALLTYPE PetDropTarget::AddRef() { return ++references_; }

ULONG STDMETHODCALLTYPE PetDropTarget::Release() {
  const ULONG remaining = --references_;
  if (remaining == 0) delete this;
  return remaining;
}

HRESULT STDMETHODCALLTYPE PetDropTarget::DragEnter(IDataObject *dataObject,
                                                   DWORD keyState,
                                                   POINTL point,
                                                   DWORD *effect) {
  (void)keyState;
  if (effect == nullptr) return E_INVALIDARG;
  impl_->setEffect(effect, false);
  try {
    if (!impl_->acceptingCallbacks() || impl_->session.waitingForAck()) {
      return S_OK;
    }
    impl_->exitHover();
    impl_->clearCandidate();
    if (!ExtractFirstOrdinaryFile(dataObject, &impl_->candidatePath)) {
      return S_OK;
    }
    impl_->candidateKind = ClassifyFile(impl_->candidatePath);
    const bool accepted = impl_->updateHover(point);
    impl_->setEffect(effect, accepted);
    return S_OK;
  } catch (...) {
    impl_->clearCandidate();
    return E_OUTOFMEMORY;
  }
}

HRESULT STDMETHODCALLTYPE PetDropTarget::DragOver(DWORD keyState, POINTL point,
                                                  DWORD *effect) {
  (void)keyState;
  if (effect == nullptr) return E_INVALIDARG;
  impl_->setEffect(effect, false);
  try {
    const bool accepted = impl_->updateHover(point);
    impl_->setEffect(effect, accepted);
    return S_OK;
  } catch (...) {
    return E_OUTOFMEMORY;
  }
}

HRESULT STDMETHODCALLTYPE PetDropTarget::DragLeave() {
  if (impl_->acceptingCallbacks()) impl_->exitHover();
  impl_->clearCandidate();
  return S_OK;
}

HRESULT STDMETHODCALLTYPE PetDropTarget::Drop(IDataObject *dataObject,
                                              DWORD keyState, POINTL point,
                                              DWORD *effect) {
  (void)keyState;
  if (effect == nullptr) return E_INVALIDARG;
  impl_->setEffect(effect, false);
  try {
    if (!impl_->acceptingCallbacks() || impl_->session.waitingForAck()) {
      return S_OK;
    }

    std::wstring submittedPath;
    if (!ExtractFirstOrdinaryFile(dataObject, &submittedPath) ||
        !impl_->pointerInside(point)) {
      impl_->exitHover();
      impl_->clearCandidate();
      return S_OK;
    }
    if (submittedPath != impl_->candidatePath) {
      impl_->exitHover();
      impl_->candidatePath = submittedPath;
      impl_->candidateKind = ClassifyFile(submittedPath);
    }
    if (!impl_->enterHover()) {
      impl_->clearCandidate();
      return S_OK;
    }
    const uint64_t generation = impl_->session.generation();
    std::string utf8Path;
    if (!Utf16ToUtf8(submittedPath, &utf8Path) ||
        !impl_->session.submit(generation, submittedPath)) {
      impl_->exitHover();
      impl_->clearCandidate();
      return S_OK;
    }
    impl_->setVisual(PetDropVisualState::WaitingForAck);
    impl_->emit(kCallbackFileDropped, utf8Path.c_str(), generation);
    impl_->clearCandidate();
    impl_->setEffect(effect, true);
    return S_OK;
  } catch (...) {
    impl_->clearCandidate();
    return E_OUTOFMEMORY;
  }
}

bool PetDropTarget::finish(uint64_t generation, uint32_t result) {
  if (!impl_->acceptingCallbacks() ||
      !impl_->session.finish(generation, result)) {
    return false;
  }
  impl_->setVisual(result == PET_DROP_ACCEPTED
                       ? PetDropVisualState::SwallowAndSuccessJet
                       : PetDropVisualState::SwallowAndEject);
  return true;
}

void PetDropTarget::cancelHover() {
  if (impl_->acceptingCallbacks()) impl_->exitHover();
  impl_->clearCandidate();
}

void PetDropTarget::deactivate() {
  if (impl_ == nullptr || !impl_->active) return;
  impl_->active = false;
  (void)impl_->session.leave();
  impl_->clearCandidate();
}
