# COVERAGE — đặc tả ↔ kế hoạch (SLUG)

> Spec: `spec.md`
> Mã đặc tả: mặc định BO-/UC-/BR-/P-XX/D-XX/W-XX/SCR- (+ US·AC = dòng **US-01** rồi bảng AC-01). Đặc tả dùng mã khác → thêm 1 dòng riêng gồm `> Codes: ` rồi regex trong cặp backtick (script chỉ đọc dòng đúng dạng đó).
> Chỉ dùng khi feature có **đặc tả ngoài** (file BA giữ). Mỗi mã trong đặc tả có đúng 1 dòng ở đây. Kiểm: `python3 ~/.omp/skills/feature/scripts/coverage.py agents/planning/SLUG`.
> Danh sách mã phải đọc của 1 phase: thêm `--phase Mx`. Cột Phase ghi `M1 · M2`, `M4–M6` (khoảng), `MC`; script đọc đúng các dạng đó.
> Trạng thái (từ đầu ô cuối): **OK** đã có chỗ trong kế hoạch · **ADDED** gap vừa bổ sung · **PROBE** chờ kiểm thử kỹ thuật đầu phase · **HUB** việc ngoài repo · **BLOCKED** chờ dữ liệu/quyết định · **GAP** chưa có chỗ (script FAIL).

## 1. Mã đặc tả (BO / UC / BR / D / W / P / SCR)

| Mã | Nội dung (tóm tắt) | Phase | Ở đâu trong kế hoạch | Trạng thái |
|---|---|---|---|---|
| UC-01 | [tóm tắt] | M1 | UC01 | OK |

## 2. Acceptance Criteria của đặc tả (theo US)

| US·AC | UC | Phase | Then (trích đặc tả) | Liên quan | Trạng thái |
|---|---|---|---|---|---|
| US-01·AC-01 | UC01 | M1 | [Then rút gọn] | Luồng chính | OK |

## 3. Gap tìm được khi đối soát

- (chưa có)
