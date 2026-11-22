# Vektora 0.2.0 benchmark

Counting: nodes = every M, L and C segment (the same counter as the baseline benchmark). MAE = mean absolute RGB error of the render against the 1200 px source (0..255, white background). Thin-line recall = share of the original's thin-line pixels (dark strokes thinner than about 2r+1 px, found by the same detector the guard uses) that the render still shows within 1 px. Time = whole pipeline on this PC (integrated AMD GPU for Real-ESRGAN).

## Illustrations (B1, B2, B4)

| file | variant | paths | nodes | KB | MAE | thin-line recall | time s |
|---|---|---:|---:|---:|---:|---:|---:|
| Orang (6 people) | baseline: VTracer on ESRGAN 2x | 61 | 1903 | 68 | 9.03 | 33% | - |
| Orang (6 people) | Vektora (AI clean) | 127 | 1558 | 65 | 8.13 | 83% | 8.8 |
| Orang (6 people) | Vektora (median, no AI) | 125 | 1764 | 76 | 7.90 | 84% | 0.6 |
| Coastal Drift | baseline: VTracer on ESRGAN 2x | 67 | 2208 | 82 | 3.64 | 95% | - |
| Coastal Drift | Vektora (AI clean) | 217 | 1614 | 79 | 4.87 | 91% | 23.0 |
| Coastal Drift | Vektora (median, no AI) | 216 | 1593 | 79 | 4.46 | 89% | 1.4 |
| Wartec | baseline: VTracer on ESRGAN 2x | 133 | 4337 | 149 | 5.83 | 69% | - |
| Wartec | Vektora (AI clean) | 315 | 2837 | 133 | 7.39 | 75% | 18.6 |
| Wartec | Vektora (median, no AI) | 315 | 2790 | 131 | 7.24 | 74% | 1.3 |
| Drifting Leaf | baseline: VTracer on ESRGAN 2x | 144 | 6159 | 231 | 5.77 | 74% | - |
| Drifting Leaf | Vektora (AI clean) | 500 | 2917 | 152 | 7.03 | 85% | 23.6 |
| Drifting Leaf | Vektora (median, no AI) | 500 | 3355 | 170 | 6.68 | 85% | 2.0 |

## Logos (B3)

| file | variant | paths | nodes | KB | MAE | thin-line recall | time s |
|---|---|---:|---:|---:|---:|---:|---:|
| Logo taekwondo | hand-made vector (owner) | 7 | 113 | 3 | 0.00 | 0% | - |
| Logo taekwondo | Vektora (AI clean) | 2 | 98 | 4 | 2.62 | 0% | 24.6 |
| Logo taekwondo | Vektora (median, no AI) | 2 | 95 | 4 | 0.91 | 0% | 1.2 |
| Logo golf | hand-made vector (owner) | 22 | 413 | 12 | 4.58 | 0% | - |
| Logo golf | Vektora (AI clean) | 34 | 682 | 28 | 3.36 | 63% | 25.8 |
| Logo golf | Vektora (median, no AI) | 34 | 671 | 28 | 2.40 | 63% | 2.1 |
| Logo Bangga | hand-made vector (owner) | 31 | 645 | 25 | 0.00 | 0% | - |
| Logo Bangga | Vektora (AI clean) | 3 | 553 | 21 | 3.01 | 0% | 32.7 |
| Logo Bangga | Vektora (median, no AI) | 3 | 548 | 21 | 1.21 | 0% | 1.0 |
| Logo Ari Reliyanto | hand-made vector (owner) | 0 | 0* | 172 | 0.00 | 100% | - |
| Logo Ari Reliyanto | Vektora (AI clean) | 12 | 163 | 7 | 2.67 | 84% | 59.7 |
| Logo Ari Reliyanto | Vektora (median, no AI) | 12 | 161 | 7 | 0.77 | 83% | 0.9 |
| Logo kos kausar | hand-made vector (owner) | 243 | 6051* | 128 | 1.02 | 80% | - |
| Logo kos kausar | Vektora (AI clean) | 350 | 2319 | 114 | 5.95 | 78% | 21.7 |
| Logo kos kausar | Vektora (median, no AI) | 350 | 2354 | 115 | 5.15 | 78% | 1.6 |
| Logo atrione taekwondo | hand-made vector (owner) | 7 | 113* | 2 | 0.00 | 0% | - |
| Logo atrione taekwondo | Vektora (AI clean) | 2 | 109 | 4 | 2.59 | 0% | 34.7 |
| Logo atrione taekwondo | Vektora (median, no AI) | 2 | 107 | 4 | 0.86 | 0% | 0.9 |

## Garis tipis sebagai stroke vs bentuk (0.2.0, tanpa AI)

| file | node total (stroke) | node total (bentuk) | node grup Garis (stroke) | node grup Garis (bentuk) |
|---|---:|---:|---:|---:|
| Orang (6 people) | 1764 | 2803 | 376 | 1415 |
| Coastal Drift | 1593 | 1521 | 471 | 399 |
| Wartec | 2790 | 3225 | 1191 | 1626 |
| Drifting Leaf | 3355 | 2899 | 1105 | 649 |
| Logo taekwondo | 95 | 95 | 0 | 0 |
| Logo golf | 671 | 729 | 86 | 144 |
| Logo Bangga | 548 | 548 | 0 | 0 |
| Logo Ari Reliyanto | 161 | 202 | 20 | 61 |
| Logo kos kausar | 2354 | 2655 | 1657 | 1958 |
| Logo atrione taekwondo | 107 | 107 | 0 | 0 |

`*` = counted from the hand-made SVG with this counter (shapes such as rect or circle are not counted).
