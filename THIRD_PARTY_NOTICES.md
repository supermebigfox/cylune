# Third-party notices

The default black-hole renderer in `src-tauri/native/mac/shader.metal` is
ported from the pinned `tiyda/blackhole-desktop` source described first below.
CYLUNE adds a movable center, live size control, multi-display capture, and a
safe print-file import target. The other entries document earlier renderer and
import-animation work that remains in the application. No unrelated timer,
terminal, trash, settings, or packaging behavior was copied.

## Windows Direct3D/HLSL port

`src-tauri/native/windows/BlackHole.hlsl` is a direct numerical HLSL port of
the sealed CYLUNE Metal black-hole shader and therefore carries forward the
same upstream-derived optics, presets, material parameters, flow, animation,
and attribution described in every existing entry below. The Windows adapter
changes the shader language and uniform/capture coordinates, samples a live
DXGI Desktop Duplication texture, and presents through Direct3D 11 and
DirectComposition; it does not replace, narrow, or remove any upstream MIT
notice or license grant.

Direct3D, DXGI, DirectComposition, DWM, OLE, Win32, and the WebView2
bootstrapper are consumed as Microsoft platform/runtime interfaces rather
than copied source from the cited black-hole projects. The complete notice
file is bundled into the Windows application as `THIRD_PARTY_NOTICES.md`.

## tiyda/blackhole-desktop

Pinned source:
https://github.com/tiyda/blackhole-desktop/tree/03e74a5cf2522748993aca679cdc6027c7b19697

The default black-hole optics, preset values, 40-step ray integration,
time-driven accretion-disk noise, transparent effect mask, and desktop
background sampling are a Metal adaptation of the pinned source. CYLUNE
replaces the source project's autonomous orbit with a user-controlled center
and connects the renderer to its own capture, sizing, and file-import systems.

MIT License

Copyright (c) 2026 tiyda

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.

## rrrjqy66/BlackHoleTrash

Pinned source:
https://github.com/rrrjqy66/BlackHoleTrash/tree/229d93213cd3e57364b4c6655cfb2c75b7ea4d18

The black-hole optics in `src-tauri/native/mac/shader.metal` are a WGSL-to-MSL
port of the pinned `src/black_hole_trash.wgsl`. The port changes shader
language, uniform layout, desktop-capture coordinates, and transparent
premultiplied composition for this application's circular panel. It retains
the pinned Gargantua preset's Schwarzschild geodesic integration,
finite-camera weak-deflection fit, multiple thin-disk crossings, blackbody
color, Doppler shift, beaming, wrapped noise, and exposure curve. Recycling,
cursor, configuration, packaging, and other unrelated product behavior was
not imported.

MIT License

Copyright (c) 2026 GreenScreen410

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.

## cabbagehao/blackhole-timer

Pinned source:
https://github.com/cabbagehao/blackhole-timer/tree/f3cc9cc349540ad6d274cd8074cf050b9b0c0200

The Fusion appearance in `src-tauri/native/mac/shader.metal` actively adapts
material parameters and mask semantics from the pinned source into the
application's existing single Schwarzschild trace. Browser and Pomodoro
behavior was not copied.

MIT License

Copyright (c) 2026 s13k <s13k@pm.me>

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.

## s0xDk/ghostty-blackhole

Source: https://github.com/s0xDk/ghostty-blackhole

MIT License

Copyright (c) 2026 s13k <s13k@pm.me>

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.

## ZGhey/blackhole-mac

Pinned source:
https://github.com/ZGhey/blackhole-mac/tree/f719aa1139ecc49a728cbb8fac2e60fcfa51996e

The procedural file-card animation in `src-tauri/native/mac/shader.metal`
adapts the pinned source's Faller radial path, orbital timing, tidal shear,
fragment staging, and Impacts attack/decay behavior. It uses locally drawn
generic 3MF and G-code cards and does not copy file icons, filenames, cursor
graphics, batch behavior, application UI, or packaging.

MIT License

Copyright (c) 2026 Jack Zhang

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
