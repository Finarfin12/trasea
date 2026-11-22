# Vektora 0.1.0 benchmark

Counting: nodes = every M, L and C segment (the same counter as the baseline benchmark). MAE = mean absolute RGB error of the render against the 1200 px source (0..255, white background). Thin-line recall = share of the original's thin-line pixels (dark strokes thinner than about 2r+1 px, found by the same detector the guard uses) that the render still shows within 1 px. Time = whole pipeline on this PC (integrated AMD GPU for Real-ESRGAN).

## Illustrations (B1, B2, B4)

| file | variant | paths | nodes | KB | MAE | thin-line recall | time s |
|---|---|---:|---:|---:|---:|---:|---:|
| Orang (6 people) | baseline: VTracer on ESRGAN 2x | 61 | 1903 | 68 | 9.03 | 33% | - |
| Orang (6 people) | Vektora (AI clean) | 10 | 3046 | 110 | 7.68 | 93% | 8.6 |
| Orang (6 people) | Vektora (median, no AI) | 8 | 2785 | 104 | 7.47 | 93% | 0.5 |
| Coastal Drift | baseline: VTracer on ESRGAN 2x | 67 | 2208 | 82 | 3.64 | 95% | - |
| Coastal Drift | Vektora (AI clean) | 9 | 1538 | 58 | 4.69 | 97% | 23.3 |
| Coastal Drift | Vektora (median, no AI) | 8 | 1590 | 60 | 4.24 | 97% | 1.2 |
| Wartec | baseline: VTracer on ESRGAN 2x | 133 | 4337 | 149 | 5.83 | 69% | - |
| Wartec | Vektora (AI clean) | 10 | 3338 | 123 | 6.60 | 82% | 20.3 |
| Wartec | Vektora (median, no AI) | 10 | 3272 | 121 | 6.41 | 82% | 1.1 |
| Drifting Leaf | baseline: VTracer on ESRGAN 2x | 144 | 6159 | 231 | 5.77 | 74% | - |
| Drifting Leaf | Vektora (AI clean) | 11 | 2422 | 92 | 6.51 | 95% | 25.1 |
| Drifting Leaf | Vektora (median, no AI) | 11 | 3495 | 134 | 6.79 | 96% | 1.5 |

## Logos (B3)

| file | variant | paths | nodes | KB | MAE | thin-line recall | time s |
|---|---|---:|---:|---:|---:|---:|---:|
| Logo taekwondo | hand-made vector (owner) | 7 | 113 | 3 | 0.00 | 0% | - |
| Logo taekwondo | Vektora (AI clean) | 2 | 98 | 4 | 2.63 | 0% | 22.7 |
| Logo taekwondo | Vektora (median, no AI) | 2 | 95 | 3 | 0.91 | 0% | 0.9 |
| Logo golf | hand-made vector (owner) | 22 | 413 | 12 | 4.58 | 0% | - |
| Logo golf | Vektora (AI clean) | 5 | 740 | 28 | 3.34 | 68% | 24.5 |
| Logo golf | Vektora (median, no AI) | 5 | 730 | 27 | 2.36 | 68% | 2.0 |
| Logo Bangga | hand-made vector (owner) | 31 | 645 | 25 | 0.00 | 0% | - |
| Logo Bangga | Vektora (AI clean) | 3 | 550 | 21 | 3.04 | 0% | 34.7 |
| Logo Bangga | Vektora (median, no AI) | 3 | 546 | 21 | 1.29 | 0% | 0.7 |
| Logo Ari Reliyanto | hand-made vector (owner) | 0 | 0* | 172 | 0.00 | 100% | - |
| Logo Ari Reliyanto | Vektora (AI clean) | 3 | 204 | 7 | 2.64 | 99% | 34.5 |
| Logo Ari Reliyanto | Vektora (median, no AI) | 3 | 203 | 7 | 0.74 | 99% | 2.3 |
| Logo kos kausar | hand-made vector (owner) | 243 | 6051* | 128 | 1.02 | 80% | - |
| Logo kos kausar | Vektora (AI clean) | 7 | 2628 | 94 | 5.29 | 87% | 33.3 |
| Logo kos kausar | Vektora (median, no AI) | 7 | 2670 | 95 | 4.50 | 87% | 0.9 |
| Logo atrione taekwondo | hand-made vector (owner) | 7 | 113* | 2 | 0.00 | 0% | - |
| Logo atrione taekwondo | Vektora (AI clean) | 2 | 107 | 4 | 2.61 | 0% | 38.8 |
| Logo atrione taekwondo | Vektora (median, no AI) | 2 | 107 | 4 | 0.89 | 0% | 1.3 |

`*` = counted from the hand-made SVG with this counter (shapes such as rect or circle are not counted).
