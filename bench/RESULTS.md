# Trasea 0.1.0 benchmark

Counting: nodes = every M, L and C segment (the same counter as the baseline benchmark). MAE = mean absolute RGB error of the render against the 1200 px source (0..255, white background). Thin-line recall = share of the original's thin-line pixels (dark strokes thinner than about 2r+1 px, found by the same detector the guard uses) that the render still shows within 1 px. Time = whole pipeline on this PC (integrated AMD GPU for Real-ESRGAN).

## Illustrations (B1, B2, B4)

| file | variant | paths | nodes | KB | MAE | thin-line recall | time s |
|---|---|---:|---:|---:|---:|---:|---:|
| Orang (6 people) | baseline: VTracer on ESRGAN 2x | 61 | 1903 | 68 | 9.03 | 29% | - |
| Orang (6 people) | Trasea (median, no AI) | 113 | 1683 | 65 | 8.15 | 89% | 0.5 |
| Coastal Drift | baseline: VTracer on ESRGAN 2x | 67 | 2208 | 82 | 3.64 | 91% | - |
| Coastal Drift | Trasea (median, no AI) | 57 | 1514 | 53 | 4.68 | 95% | 1.3 |
| Wartec | baseline: VTracer on ESRGAN 2x | 133 | 4337 | 149 | 5.83 | 64% | - |
| Wartec | Trasea (median, no AI) | 205 | 3122 | 111 | 7.32 | 80% | 1.3 |
| Drifting Leaf | baseline: VTracer on ESRGAN 2x | 144 | 6159 | 231 | 5.77 | 74% | - |
| Drifting Leaf | Trasea (median, no AI) | 126 | 2469 | 86 | 6.91 | 95% | 2.1 |

## Logos (B3)

| file | variant | paths | nodes | KB | MAE | thin-line recall | time s |
|---|---|---:|---:|---:|---:|---:|---:|
| Logo taekwondo | hand-made vector (owner) | 7 | 113 | 3 | 0.00 | 0% | - |
| Logo taekwondo | Trasea (median, no AI) | 2 | 94 | 2 | 0.38 | 0% | 1.3 |
| Logo golf | hand-made vector (owner) | 22 | 413 | 12 | 4.58 | 0% | - |
| Logo golf | Trasea (median, no AI) | 26 | 534 | 14 | 2.24 | 76% | 1.9 |
| Logo Bangga | hand-made vector (owner) | 31 | 645 | 25 | 0.00 | 0% | - |
| Logo Bangga | Trasea (median, no AI) | 3 | 429 | 15 | 1.22 | 0% | 0.9 |
| Logo Ari Reliyanto | hand-made vector (owner) | 0 | 0* | 172 | 0.00 | 100% | - |
| Logo Ari Reliyanto | Trasea (median, no AI) | 15 | 220 | 7 | 0.69 | 95% | 1.1 |
| Logo kos kausar | hand-made vector (owner) | 243 | 6119* | 128 | 1.02 | 81% | - |
| Logo kos kausar | Trasea (median, no AI) | 320 | 2364 | 92 | 4.82 | 87% | 1.2 |
| Logo atrione taekwondo | hand-made vector (owner) | 7 | 117* | 2 | 0.00 | 0% | - |
| Logo atrione taekwondo | Trasea (median, no AI) | 2 | 96 | 2 | 0.35 | 0% | 0.8 |

## 0.2.0 vs 0.3.0 (tanpa AI)

| file | path di layer Garis 0.2.0 | path di layer Garis 0.3.0 | node total 0.2.0 | node total 0.3.0 | bentuk dasar | area teks terdeteksi | simetri |
|---|---:|---:|---:|---:|---:|---:|---|
| Orang (6 people) | - | 108 | - | 1683 | 2 | 2 | - |
| Coastal Drift | - | 53 | - | 1514 | 0 | 1 | - |
| Wartec | - | 198 | - | 3122 | 1 | 6 | - |
| Drifting Leaf | - | 119 | - | 2469 | 2 | 0 | - |
| Logo taekwondo | - | 0 | - | 94 | 2 | 0 | - |
| Logo golf | - | 23 | - | 534 | 0 | 2 | - |
| Logo Bangga | - | 0 | - | 429 | 0 | 2 | - |
| Logo Ari Reliyanto | - | 13 | - | 220 | 2 | 3 | - |
| Logo kos kausar | - | 316 | - | 2364 | 2 | 2 | - |
| Logo atrione taekwondo | - | 0 | - | 96 | 2 | 0 | - |

Waktu total seluruh gambar benchmark (semua varian yang dijalankan): 12.3 detik.

## Garis tipis sebagai stroke vs bentuk (0.3.0, tanpa AI)

| file | node total (stroke) | node total (bentuk) | node grup Garis (stroke) | node grup Garis (bentuk) |
|---|---:|---:|---:|---:|
| Orang (6 people) | 1683 | 2990 | 478 | 1785 |
| Coastal Drift | 1514 | 1700 | 672 | 858 |
| Wartec | 3122 | 3610 | 1868 | 2356 |
| Drifting Leaf | 2469 | 2762 | 622 | 915 |
| Logo taekwondo | 94 | 94 | 0 | 0 |
| Logo golf | 534 | 609 | 66 | 141 |
| Logo Bangga | 429 | 429 | 0 | 0 |
| Logo Ari Reliyanto | 220 | 266 | 60 | 106 |
| Logo kos kausar | 2364 | 3160 | 1773 | 2569 |
| Logo atrione taekwondo | 96 | 96 | 0 | 0 |

`*` = counted from the hand-made SVG with this counter (shapes such as rect or circle are not counted).
