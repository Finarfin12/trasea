# Vektora 0.1.0 benchmark

Counting: nodes = every M, L and C segment (the same counter as the baseline benchmark). MAE = mean absolute RGB error of the render against the 1200 px source (0..255, white background). Thin-line recall = share of the original's thin-line pixels (dark strokes thinner than about 2r+1 px, found by the same detector the guard uses) that the render still shows within 1 px. Time = whole pipeline on this PC (integrated AMD GPU for Real-ESRGAN).

## Illustrations (B1, B2, B4)

| file | variant | paths | nodes | KB | MAE | thin-line recall | time s |
|---|---|---:|---:|---:|---:|---:|---:|
| Orang (6 people) | baseline: VTracer on ESRGAN 2x | 61 | 1903 | 68 | 9.03 | 33% | - |
| Orang (6 people) | Vektora (AI clean) | 318 | 2811 | 127 | 7.95 | 77% | 8.6 |
| Orang (6 people) | Vektora (median, no AI) | 316 | 2550 | 121 | 7.75 | 80% | 0.6 |
| Coastal Drift | baseline: VTracer on ESRGAN 2x | 67 | 2208 | 82 | 3.64 | 95% | - |
| Coastal Drift | Vektora (AI clean) | 581 | 2338 | 137 | 5.02 | 89% | 23.1 |
| Coastal Drift | Vektora (median, no AI) | 580 | 2390 | 140 | 4.57 | 89% | 1.2 |
| Wartec | baseline: VTracer on ESRGAN 2x | 133 | 4337 | 149 | 5.83 | 69% | - |
| Wartec | Vektora (AI clean) | 594 | 3544 | 182 | 7.16 | 82% | 18.1 |
| Wartec | Vektora (median, no AI) | 594 | 3478 | 180 | 6.97 | 83% | 1.2 |
| Drifting Leaf | baseline: VTracer on ESRGAN 2x | 144 | 6159 | 231 | 5.77 | 74% | - |
| Drifting Leaf | Vektora (AI clean) | 1833 | 5526 | 365 | 6.75 | 88% | 22.7 |
| Drifting Leaf | Vektora (median, no AI) | 1833 | 6599 | 407 | 7.01 | 88% | 1.7 |

## Logos (B3)

| file | variant | paths | nodes | KB | MAE | thin-line recall | time s |
|---|---|---:|---:|---:|---:|---:|---:|
| Logo taekwondo | hand-made vector (owner) | 7 | 113 | 3 | 0.00 | 0% | - |
| Logo taekwondo | Vektora (AI clean) | 2 | 98 | 4 | 2.63 | 0% | 21.9 |
| Logo taekwondo | Vektora (median, no AI) | 2 | 95 | 4 | 0.91 | 0% | 0.8 |
| Logo golf | hand-made vector (owner) | 22 | 413 | 12 | 4.58 | 0% | - |
| Logo golf | Vektora (AI clean) | 61 | 736 | 33 | 3.39 | 66% | 23.1 |
| Logo golf | Vektora (median, no AI) | 61 | 726 | 32 | 2.41 | 66% | 1.6 |
| Logo Bangga | hand-made vector (owner) | 31 | 645 | 25 | 0.00 | 0% | - |
| Logo Bangga | Vektora (AI clean) | 3 | 550 | 21 | 3.04 | 0% | 30.0 |
| Logo Bangga | Vektora (median, no AI) | 3 | 546 | 21 | 1.29 | 0% | 0.7 |
| Logo Ari Reliyanto | hand-made vector (owner) | 0 | 0* | 172 | 0.00 | 100% | - |
| Logo Ari Reliyanto | Vektora (AI clean) | 16 | 170 | 7 | 2.71 | 87% | 29.9 |
| Logo Ari Reliyanto | Vektora (median, no AI) | 16 | 169 | 7 | 0.81 | 86% | 1.0 |
| Logo kos kausar | hand-made vector (owner) | 243 | 6051* | 128 | 1.02 | 80% | - |
| Logo kos kausar | Vektora (AI clean) | 624 | 2919 | 160 | 6.05 | 72% | 20.1 |
| Logo kos kausar | Vektora (median, no AI) | 624 | 2961 | 161 | 5.27 | 73% | 1.0 |
| Logo atrione taekwondo | hand-made vector (owner) | 7 | 113* | 2 | 0.00 | 0% | - |
| Logo atrione taekwondo | Vektora (AI clean) | 2 | 107 | 4 | 2.61 | 0% | 29.7 |
| Logo atrione taekwondo | Vektora (median, no AI) | 2 | 107 | 4 | 0.89 | 0% | 0.7 |

`*` = counted from the hand-made SVG with this counter (shapes such as rect or circle are not counted).
