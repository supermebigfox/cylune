#ifndef NOMINMAX
#define NOMINMAX
#endif

#include <windows.h>
#include <d3dcompiler.h>

#include <cstdio>

namespace {

bool CompileEntry(const wchar_t *path, const char *entry, const char *target) {
  ID3DBlob *shader = nullptr;
  ID3DBlob *errors = nullptr;
  const HRESULT result = D3DCompileFromFile(
      path, nullptr, D3D_COMPILE_STANDARD_FILE_INCLUDE, entry, target,
      D3DCOMPILE_ENABLE_STRICTNESS | D3DCOMPILE_WARNINGS_ARE_ERRORS, 0,
      &shader, &errors);
  if (errors != nullptr) {
    std::fwrite(errors->GetBufferPointer(), 1, errors->GetBufferSize(), stderr);
    errors->Release();
  }
  if (shader != nullptr) shader->Release();
  return SUCCEEDED(result);
}

}  // namespace

int wmain(int argumentCount, wchar_t **arguments) {
  if (argumentCount != 2) return 2;
  if (!CompileEntry(arguments[1], "vs_main", "vs_5_0")) return 3;
  if (!CompileEntry(arguments[1], "ps_main", "ps_5_0")) return 4;
  return 0;
}
