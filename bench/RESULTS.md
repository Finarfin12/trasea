# Trasea 0.1.0 benchmark

Counting: nodes = every M, L and C segment (the same counter as the baseline benchmark). MAE = mean absolute RGB error of the render against the 1200 px source (0..255, white background). Thin-line recall = share of the original's thin-line pixels (dark strokes thinner than about 2r+1 px, found by the same detector the guard uses) that the render still shows within 1 px. Time = whole pipeline on this PC (integrated AMD GPU for Real-ESRGAN).

## Illustrations (B1, B2, B4)

| file | variant | paths | nodes | KB | MAE | thin-line recall | time s |
|---|---|---:|---:|---:|---:|---:|---:|
| Orang (6 people) | baseline: VTracer on ESRGAN 2x | 61 | 1903 | 68 | 9.03 | 32% | - |
| Orang (6 people) | Trasea (median, no AI) | 106 | 1621 | 62 | 8.01 | 86% | 0.7 |
| Coastal Drift | baseline: VTracer on ESRGAN 2x | 67 | 2208 | 82 | 3.64 | 93% | - |
| Coastal Drift | Trasea (median, no AI) | 95 | 1346 | 52 | 4.59 | 87% | 1.9 |
| Wartec | baseline: VTracer on ESRGAN 2x | 133 | 4337 | 149 | 5.83 | 64% | - |
| Wartec | Trasea (median, no AI) | 254 | 3033 | 113 | 7.29 | 77% | 1.8 |
| Drifting Leaf | baseline: VTracer on ESRGAN 2x | 144 | 6159 | 231 | 5.77 | 73% | - |
| Drifting Leaf | Trasea (median, no AI) | 158 | 2427 | 88 | 6.85 | 87% | 2.7 |

## Logos (B3)

| file | variant | paths | nodes | KB | MAE | thin-line recall | time s |
|---|---|---:|---:|---:|---:|---:|---:|
| Logo taekwondo | hand-made vector (owner) | 7 | 113 | 3 | 0.00 | 0% | - |
| Logo taekwondo | Trasea (median, no AI) | 2 | 94 | 2 | 0.38 | 0% | 1.6 |
| Logo golf | hand-made vector (owner) | 22 | 413 | 12 | 4.58 | 0% | - |
| Logo golf | Trasea (median, no AI) | 20 | 558 | 15 | 2.26 | 64% | 3.6 |
| Logo Bangga | hand-made vector (owner) | 31 | 645 | 25 | 0.00 | 0% | - |
| Logo Bangga | Trasea (median, no AI) | 3 | 429 | 15 | 1.22 | 0% | 1.3 |
| Logo Ari Reliyanto | hand-made vector (owner) | 0 | 0* | 172 | 0.00 | 100% | - |
| Logo Ari Reliyanto | Trasea (median, no AI) | 12 | 163 | 6 | 0.69 | 83% | 1.9 |
| Logo kos kausar | hand-made vector (owner) | 243 | 6119* | 128 | 1.02 | 81% | - |
| Logo kos kausar | Trasea (median, no AI) | 394 | 2254 | 97 | 5.02 | 79% | 2.2 |
| Logo atrione taekwondo | hand-made vector (owner) | 7 | 117* | 2 | 0.00 | 0% | - |
| Logo atrione taekwondo | Trasea (median, no AI) | 2 | 96 | 2 | 0.35 | 0% | 1.6 |

## 0.2.0 vs 0.3.0 (tanpa AI)

| file | path di layer Garis 0.2.0 | path di layer Garis 0.3.0 | node total 0.2.0 | node total 0.3.0 | bentuk dasar | area teks terdeteksi | simetri |
|---|---:|---:|---:|---:|---:|---:|---|
| Orang (6 people) | - | 101 | - | 1621 | 2 | 1 | - |
| Coastal Drift | - | 90 | - | 1346 | 1 | 1 | - |
| Wartec | - | 247 | - | 3033 | 1 | 8 | - |
| Drifting Leaf | - | 151 | - | 2427 | 2 | 1 | - |
| Logo taekwondo | - | 0 | - | 94 | 2 | 0 | - |
| Logo golf | - | 17 | - | 558 | 0 | 2 | - |
| Logo Bangga | - | 0 | - | 429 | 0 | 2 | - |
| Logo Ari Reliyanto | - | 10 | - | 163 | 2 | 2 | - |
| Logo kos kausar | - | 390 | - | 2254 | 2 | 2 | - |
| Logo atrione taekwondo | - | 0 | - | 96 | 2 | 0 | - |

Waktu total seluruh gambar benchmark (semua varian yang dijalankan): 19.2 detik.

## Garis tipis sebagai stroke vs bentuk (0.3.0, tanpa AI)

| file | node total (stroke) | node total (bentuk) | node grup Garis (stroke) | node grup Garis (bentuk) |
|---|---:|---:|---:|---:|
| Orang (6 people) | 1621 | 2831 | 413 | 1623 |
| Coastal Drift | 1346 | 1476 | 428 | 558 |
| Wartec | 3033 | 3544 | 1706 | 2217 |
| Drifting Leaf | 2427 | 2699 | 470 | 742 |
| Logo taekwondo | 94 | 94 | 0 | 0 |
| Logo golf | 558 | 603 | 98 | 143 |
| Logo Bangga | 429 | 429 | 0 | 0 |
| Logo Ari Reliyanto | 163 | 215 | 20 | 72 |
| Logo kos kausar | 2254 | 2818 | 1660 | 2224 |
| Logo atrione taekwondo | 96 | 96 | 0 | 0 |

`*` = counted from the hand-made SVG with this counter (shapes such as rect or circle are not counted).
