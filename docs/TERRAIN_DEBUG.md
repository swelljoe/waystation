# Terrain Debugging Language

Press **F3** in the game to show both logical terrain cells and rendered
dual-grid tiles around the camera. Yellow logical-cell markers look like
`W28,23 D`. Rendered-tile labels have this form:

```text
R30,25 M0D
DG/DD
A15 G4,3
DG.except_ne
```

- `W28,23 D` is logical world cell `(28,23)`, containing dirt.
- `R30,25` is rendered dual-grid tile `(30,25)`. It draws the shared
  corner of logical cells `W28,24`, `W29,24`, `W28,23`, and `W29,23`.
- `M0D` is the hexadecimal dirt mask. Bits are NW=`1`, NE=`2`, SE=`4`, and
  SW=`8`.
- `DG/DD` displays that same mask spatially as `NW NE / SW SE`: the northeast
  logical cell is grass and the other three are dirt.
- `A15` is runtime terrain-atlas slot 15.
- `G4,3` is column 4, row 3 in `THE GROUND/The Ground - 1-1.png`, measured
  from the source image's top-left corner.
- `DG.except_ne` is the semantic role assigned to that atlas slot.

A useful report is: “`R(30,25) M0D` currently uses `A15 / G(4,3)`, but it
should use `G(5,2)`.” The logical `W` markers can be used to report an incorrect
terrain value; the rendered `R` labels identify an incorrect art mapping.

## Dual-grid layout

Logical cells are centered half a tile northeast of the rendered-tile origins.
Consequently, the middle of a one-cell-wide logical path lies where four
rendered sprites meet. Specifically, `W(x,y)` is shared by rendered tiles
`R(x+1,y+1)`, `R(x+2,y+1)`, `R(x+1,y+2)`, and `R(x+2,y+2)`. The generator
removes the two ambiguous checkerboard masks (`M05` and `M0A`) because this art
set intentionally has no matching pieces.

## Atlas catalog

| Atlas | Role | THE GROUND source |
|---:|---|---:|
| A0 | `G.plain` | G(0,0) |
| A1 | `G.plain_alt` | G(0,0) |
| A2 | `G.flowers` | G(0,1) |
| A3 | `G.rocks` | G(1,0) |
| A4 | `DG.full_a` | G(0,2) |
| A5 | `DG.full_b` | G(0,2) |
| A6 | `DG.only_nw` | G(5,1) |
| A7 | `DG.only_ne` | G(5,2) |
| A8 | `DG.only_se` | G(4,0) |
| A9 | `DG.only_sw` | G(5,0) |
| A10 | `DG.north` | G(6,2) |
| A11 | `DG.east` | G(7,2) |
| A12 | `DG.south` | G(6,3) |
| A13 | `DG.west` | G(7,3) |
| A14 | `DG.except_nw` | G(5,3) |
| A15 | `DG.except_ne` | G(4,3) |
| A16 | `DG.except_se` | G(4,2) |
| A17 | `DG.except_sw` | G(4,1) |
| A18 | `W.center` | G(40,0) |
| A19 | `W.isolated` | G(32,1) |
| A20 | `W.isolated_small` | G(33,1) |
| A21 | `W.cap_w` | G(32,0) |
| A22 | `W.cap_e` | G(33,0) |
| A23 | `W.outer_nw` | G(34,0) |
| A24 | `W.outer_ne` | G(35,0) |
| A25 | `W.outer_sw` | G(34,1) |
| A26 | `W.outer_se` | G(35,1) |
| A27 | `W.edge_n` | G(38,1) |
| A28 | `W.edge_e` | G(36,0) |
| A29 | `W.edge_s` | G(38,0) |
| A30 | `W.edge_w` | G(36,1) |
| A31 | `W.inner_nw` | G(37,1) |
| A32 | `W.inner_ne` | G(39,1) |
| A33 | `W.inner_se` | G(37,0) |
| A34 | `W.inner_sw` | G(39,0) |

The atlas slot order is deliberately stable across licensed and public-fallback
builds. `G(column,row)` only applies to builds using `THE GROUND`; the semantic
role and `A##` remain valid for fallback art.
