# Vektora 0.1.0 benchmark

Counting: nodes = every M, L and C segment (the same counter as the baseline benchmark). MAE = mean absolute RGB error of the render against the 1200 px source (0..255, white background). Thin-line recall = share of the original's thin-line pixels (dark strokes thinner than about 2r+1 px, found by the same detector the guard uses) that the render still shows within 1 px. Time = whole pipeline on this PC (integrated AMD GPU for Real-ESRGAN).

## Illustrations (B1, B2, B4)

| file | variant | paths | nodes | KB | MAE | thin-line recall | time s |
|---|---|---:|---:|---:|---:|---:|---:|
| Orang (6 people) | baseline: VTracer on ESRGAN 2x | 61 | 1903 | 68 | 9.03 | 32% | - |
| Orang (6 people) | Vektora (AI clean) | 107 | 1383 | 53 | 8.08 | 84% | 33.7 |
| Orang (6 people) | Vektora (median, no AI) | 106 | 1726 | 65 | 8.01 | 86% | 0.6 |
| Coastal Drift | baseline: VTracer on ESRGAN 2x | 67 | 2208 | 82 | 3.64 | 93% | - |
| Coastal Drift | Vektora (AI clean) | 95 | 1541 | 57 | 5.01 | 88% | 99.2 |
| Coastal Drift | Vektora (median, no AI) | 95 | 1517 | 57 | 4.63 | 87% | 1.6 |
| Wartec | baseline: VTracer on ESRGAN 2x | 133 | 4337 | 149 | 5.83 | 64% | - |
| Wartec | Vektora (AI clean) | 254 | 3303 | 121 | 7.45 | 77% | 84.8 |
| Wartec | Vektora (median, no AI) | 254 | 3313 | 121 | 7.32 | 77% | 1.3 |
| Drifting Leaf | baseline: VTracer on ESRGAN 2x | 144 | 6159 | 231 | 5.77 | 73% | - |
| Drifting Leaf | Vektora (AI clean) | 159 | 2329 | 80 | 6.76 | 88% | 104.2 |
| Drifting Leaf | Vektora (median, no AI) | 158 | 2562 | 92 | 6.86 | 87% | 2.2 |

## Logos (B3)

| file | variant | paths | nodes | KB | MAE | thin-line recall | time s |
|---|---|---:|---:|---:|---:|---:|---:|
| Logo taekwondo | hand-made vector (owner) | 7 | 113 | 3 | 0.00 | 0% | - |
| Logo taekwondo | Vektora (AI clean) | 2 | 94 | 2 | 2.19 | 0% | 104.3 |
| Logo taekwondo | Vektora (median, no AI) | 2 | 94 | 2 | 0.38 | 0% | 1.9 |
| Logo golf | hand-made vector (owner) | 22 | 413 | 12 | 4.58 | 0% | - |
| Logo golf | Vektora (AI clean) | 20 | 673 | 18 | 3.25 | 62% | 105.3 |
| Logo golf | Vektora (median, no AI) | 20 | 666 | 17 | 2.26 | 64% | 2.1 |
| Logo Bangga | hand-made vector (owner) | 31 | 645 | 25 | 0.00 | 0% | - |
| Logo Bangga | Vektora (AI clean) | 3 | 543 | 19 | 3.03 | 0% | 139.5 |
| Logo Bangga | Vektora (median, no AI) | 3 | 537 | 19 | 1.26 | 0% | 1.1 |
| Logo Ari Reliyanto | hand-made vector (owner) | 0 | 0* | 172 | 0.00 | 100% | - |
| Logo Ari Reliyanto | Vektora (AI clean) | 12 | 167 | 6 | 2.60 | 83% | 141.7 |
| Logo Ari Reliyanto | Vektora (median, no AI) | 12 | 163 | 6 | 0.69 | 83% | 1.0 |
| Logo kos kausar | hand-made vector (owner) | 243 | 6119* | 128 | 1.02 | 81% | - |
| Logo kos kausar | Vektora (AI clean) | 394 | 2319 | 98 | 5.83 | 79% | 90.8 |
| Logo kos kausar | Vektora (median, no AI) | 394 | 2349 | 100 | 5.03 | 79% | 1.1 |
| Logo atrione taekwondo | hand-made vector (owner) | 7 | 117* | 2 | 0.00 | 0% | - |
| Logo atrione taekwondo | Vektora (AI clean) | 2 | 97 | 2 | 2.14 | 0% | 138.2 |
| Logo atrione taekwondo | Vektora (median, no AI) | 2 | 96 | 2 | 0.35 | 0% | 0.9 |

## 0.2.0 vs 0.3.0 (tanpa AI)

| file | path di layer Garis 0.2.0 | path di layer Garis 0.3.0 | node total 0.2.0 | node total 0.3.0 | bentuk dasar | area teks terdeteksi | simetri |
|---|---:|---:|---:|---:|---:|---:|---|
| Orang (6 people) | 120 | 101 | 1764 | 1726 | 2 | 1 | - |
| Coastal Drift | 211 | 90 | 1593 | 1517 | 1 | 0 | - |
| Wartec | 308 | 247 | 2790 | 3313 | 1 | 4 | - |
| Drifting Leaf | 492 | 151 | 3355 | 2562 | 2 | 1 | - |
| Logo taekwondo | 0 | 0 | 95 | 94 | 2 | 0 | - |
| Logo golf | 31 | 17 | 671 | 666 | 0 | 3 | - |
| Logo Bangga | 0 | 0 | 548 | 537 | 0 | 2 | - |
| Logo Ari Reliyanto | 10 | 10 | 161 | 163 | 2 | 2 | - |
| Logo kos kausar | 346 | 390 | 2354 | 2349 | 2 | 2 | - |
| Logo atrione taekwondo | 0 | 0 | 107 | 96 | 2 | 0 | - |

Waktu total seluruh gambar benchmark (semua varian yang dijalankan): 1055.4 detik.

## Garis tipis sebagai stroke vs bentuk (0.3.0, tanpa AI)

| file | node total (stroke) | node total (bentuk) | node grup Garis (stroke) | node grup Garis (bentuk) |
|---|---:|---:|---:|---:|
| Orang (6 people) | 1726 | 2936 | 413 | 1623 |
| Coastal Drift | 1517 | 1647 | 428 | 558 |
| Wartec | 3313 | 3824 | 1706 | 2217 |
| Drifting Leaf | 2562 | 2834 | 470 | 742 |
| Logo taekwondo | 94 | 94 | 0 | 0 |
| Logo golf | 666 | 711 | 98 | 143 |
| Logo Bangga | 537 | 537 | 0 | 0 |
| Logo Ari Reliyanto | 163 | 215 | 20 | 72 |
| Logo kos kausar | 2349 | 2913 | 1660 | 2224 |
| Logo atrione taekwondo | 96 | 96 | 0 | 0 |

`*` = counted from the hand-made SVG with this counter (shapes such as rect or circle are not counted).
