---
description: "PHP 8.3 rules có mã (C class, F input/output hàm, P kiểm tra trước khi gọi, V biến, E lỗi, R builtin 8.3, S cú pháp 8.x). BẮT BUỘC khi code PHP; là tiêu chí review (trích mã rule trong finding). Rút gọn từ docs/php83-coding-rules.md; tự đủ khi repo không có doc."
scope: "tool:edit(**/*.php), tool:write(**/*.php), tool:edit(**/*.inc), tool:write(**/*.inc), tool:edit(**/*.service), tool:write(**/*.service)"
condition: ".*"
---

# Rules: viết code PHP 8.3

Rút gọn từ `docs/php83-coding-rules.md` (bỏ ví dụ, checklist, cột hậu quả); repo không có doc (vd pms2019) → file này là nguồn đủ, mã rule tự đứng. Runtime **PHP 8.3** (D-019). Phạm vi: code **mới** và **đoạn code được sửa**; không sửa format hàng loạt code cũ. **PHẢI** / **KHÔNG** = bắt buộc; vi phạm là finding khi review, ghi mã rule (vd `F7`). Cột "Trên 8.3" = hành vi đã chạy thử: **request chết**.

## C. Khai báo class

| Mã | Rule |
|---|---|
| **C1** | Thứ tự PHẢI là: hằng → thuộc tính static → thuộc tính → `__construct` → hàm static tạo instance (`getInstance`, `fromRow`) → `public` → `protected` → `private`. |
| **C2** | Hằng PHẢI có phạm vi + kiểu: `public const string STATUS_ACTIVE = 'Active';`. Giá trị cứng (trạng thái, loại, giới hạn) PHẢI là hằng, KHÔNG lặp chuỗi ở nhiều chỗ. |
| **C3** | Thuộc tính PHẢI có phạm vi + kiểu + giá trị mặc định, trừ khi constructor luôn gán. KHÔNG `var $x;`, KHÔNG thuộc tính thiếu kiểu trong class mới. Trên 8.3: đọc khi chưa gán → `Error`; gán `''`/`null` vào `int` → `TypeError` (`'12'` tự thành 12); gán lại `readonly` → `Error`. |
| **C4** | Thuộc tính `array` PHẢI có `@var`: `list<int>`, `array<int, string>`, `array{id: int, name: string}`. |
| **C5** | Thuộc tính chỉ gán trong constructor → `readonly`. Class nhận phụ thuộc / giá trị đơn giản → constructor promotion. |
| **C6** | KHÔNG tạo thuộc tính động (`$this->x = …` khi chưa khai báo). Class legacy đang dựa vào thuộc tính động thì giữ `#[\AllowDynamicProperties]`, KHÔNG gỡ. |
| **C7** | Model vtiger (`Vtiger_Base_Model`, `Vtiger_Record_Model`): dữ liệu record đọc/ghi qua `get()` / `set()`. KHÔNG khai báo thuộc tính trùng tên field; thuộc tính chỉ dành cho state riêng của class. |
| **C8** | Thuộc tính `static` chỉ dùng làm cache **có khoá**, có kiểu: `private static array $cache = [];`. KHÔNG singleton bỏ qua tham số (instance của user 7 bị trả cho user 9). |
| **C9** | Helper / service / utils mới không có lớp con → `final class`. KHÔNG thêm hàm global mới vào `include/utils/*`; đặt thành method static trong helper của module. |
| **C10** | Override PHẢI tương thích chữ ký class cha: cùng số tham số, tham số cha có default thì con cũng PHẢI có default. Bỏ default (`f($a, $b)` override `f($a, $b = 1)`) → **fatal ngay khi nạp class**. |

## F. Hàm: input / output

| Mã | Rule |
|---|---|
| **F1** | PHẢI ghi phạm vi (`public` / `protected` / `private`), kiểu **mọi** tham số, kiểu trả về (`void` khi không trả). `static` chỉ khi hàm không dùng `$this`. Gate PHPStan tự chặn hàm mới / tham số mới thiếu kiểu (`missingType.parameter`, `missingType.return`). |
| **F2** | Docblock: một dòng mô tả (tiếng Anh); `@param` / `@return` khi kiểu PHP chưa nói đủ (mảng, ý nghĩa của `null`); `@throws` khi hàm ném lỗi. |
| **F3** | Kiểu tham số: `int` / `string` / `bool` / `float` / `array` / class / `?T`. Union chỉ khi thật sự nhận hai kiểu. KHÔNG `mixed` trừ khi chỉ chuyển tiếp. Tham số bắt buộc trước, tuỳ chọn sau. |
| **F4** | **Nơi gọi PHẢI ép kiểu trước khi truyền.** Truyền `''` / `null` vào tham số `int` là `TypeError` (file không `strict_types`); `"12abc"` cũng `TypeError`. |
| **F5** | KHÔNG thêm kiểu vào hàm cũ khi chưa rà hết nơi gọi (`lsp references` / grep) và lớp con. Một nơi gọi truyền `''` / `null` = lỗi chết mới. Thêm kiểu trả về ở lớp cha mà lớp con override chưa có kiểu → **fatal khi nạp class**; thêm kiểu tham số ở lớp con khi lớp cha chưa có → **fatal**. `: void` mà nơi gọi đang dùng kết quả → hàm đó đang có lỗi logic, sửa nơi gọi trước. |
| **F6** | Hơn 4 tham số → mảng có shape (`@param array{from: string, to: string, ids: list<int>} $filter`) hoặc class giá trị. KHÔNG gán lại tham số (copy sang biến mới). KHÔNG thêm tham số tham chiếu `&$x` mới. |
| **F7** | Giá trị trả về: không tìm thấy → `null` (`?T`); danh sách rỗng → `[]`; có/không → `bool`; sự cố → `throw`. Code mới KHÔNG trả `array\|false`, `string\|false`. |
| **F8** | **Mọi nhánh** PHẢI trả đúng kiểu khai báo (hàm `: string` mà `return null` → `TypeError`). Tham số / giá trị trả về kiểu `array` PHẢI ghi nội dung: `@param list<int> $ids`, `@return array{success: bool, message: string}`, `@return list<array{id: int, label: string}>` — gate tự chặn khi thiếu (`missingType.iterableValue`). Ghi shape thật, KHÔNG lách bằng `array<mixed>`: chỉ shape thật mới giúp PHPStan bắt nơi gọi đọc key không tồn tại. |
| **F9** | API handler / hàm facade cũ PHẢI giữ nguyên định dạng response (`['success' => …]`, `setResponse(...)`): chỉ sửa bên trong, KHÔNG đổi contract với client. Helper KHÔNG trả HTML, KHÔNG trả chuỗi đã dịch. |
| **F10** | Kiểm tra đầu vào ở đầu hàm, `return` sớm, không lồng `if` sâu. KHÔNG biến `global` mới (đọc `$adb`, `$current_user`, `$log` sẵn có được; code mới ưu tiên `PearDatabase::getInstance()`, `Users_Record_Model::getCurrentUserModel()`). KHÔNG `@` nuốt lỗi (trừ `@unlink`), `extract`, `compact`, `$$name`, `eval`. |
| **F11** | Không đổi chữ ký hàm public chỉ để "hiện đại hoá". Đổi chữ ký = đổi contract → sửa mọi nơi gọi trong cùng MR. |

## P. Kiểm tra trước khi gọi

**P0.** Kiểm **một lần** ở chỗ dữ liệu đi vào (đầu action, đầu hàm public), rồi dùng thẳng. KHÔNG lặp cùng một kiểm tra ở nhiều tầng.

| Mã | Trước khi gọi | PHẢI kiểm |
|---|---|---|
| **P1** | `Vtiger_Record_Model::getInstanceById($id, $module)` | `$id > 0 && isRecordExists($id)`; id từ request thêm `getSalesEntityType($id) === $module` |
| **P2** | Đọc / sửa / xoá record | `Users_Privileges_Model::isPermitted($module, 'DetailView' \| 'EditView' \| 'Delete', $id)` |
| **P3** | `Vtiger_Module_Model::getInstance($name)` | `instanceof Vtiger_Module_Model`; module có thể tắt → `vtlib_isModuleActive($name)` |
| **P4** | `->method()` trên giá trị có thể `null` / `false` | `instanceof` hoặc `!== null` |
| **P5** | `fetchByAssoc` / `num_rows` / `query_result` sau `pquery` | `if (!$result)` |
| **P6** | Dùng `$row` từ `fetchByAssoc()` | `if (!$row)` |
| **P7** | Key mảng từ request / DB / API / JSON | `$row['key'] ?? $default` |
| **P8** | `json_decode($json, true)` | `is_array($decoded)` |
| **P9** | `strtotime()`, `DateTime::createFromFormat()` | `!== false` |
| **P10** | `fopen`, `fsockopen`, `curl_init`, `file_get_contents` | `!== false` |
| **P11** | `fread($fp, $len)` | `$len > 0` (file rỗng, Range ngược) |
| **P12** | `/`, `%`, `intdiv` | mẫu số `!= 0` |
| **P13** | Toán trên giá trị từ DB / request | đã ép `(int)` / `(float)` |
| **P14** | `count`, `in_array`, `implode`, `array_*` | đã là mảng (`is_array` hoặc `?: []` lúc gán) |
| **P15** | Gọi API bên ngoài | timeout; HTTP status; key trong response tồn tại |
| **P16** | `new $cls`, `$obj->$method()` | `class_exists`, `method_exists`, `is_callable` |
| **P17** | Config (`$GLOBALS[...]`, `vglobal()`, `config_override`) | `?? $default` |
| **P18** | User hiện tại trong cron / webhook | `Users_Record_Model::getCurrentUserModel()` có id |

Mẫu điểm vào (action): biến dùng trong `catch` gán trước `try` (V1); kiểm id + module + quyền trước `getInstanceById` (P1/P2); `catch (Throwable $e)` → `saveLog(...)` kèm id/module → `setError(vtranslate(...))`, không trả message thô (E1/E2).

## V. Biến

| Mã | Rule | Sai → Đúng |
|---|---|---|
| **V1** | Khởi tạo trước khi dùng, đúng kiểu cuối cùng. KHÔNG định nghĩa trong `if` / vòng lặp rồi dùng ở ngoài. Biến dùng trong `catch` PHẢI gán trước `try`. | `if ($x) { $rows = …; } return $rows;` → `$rows = []; if ($x) { … }` |
| **V2** | Mỗi biến một kiểu; tên nói lên kiểu: `$recordId` (int), `$recordModel` (object), `$rows` / `$recordIds` (danh sách), `$isActive` / `$hasPermission` (bool), `$amount` (float). | `$data` lúc chuỗi lúc mảng |
| **V3** | Ép kiểu **ngay khi gán** dữ liệu từ request / DB / API, sau đó dùng thẳng. Tiền / giờ / % → `(float)`. KHÔNG `(array)` cho danh sách id. | `(array) $request->get('ids')` (`''` → `['']` → id 0 giả) → `array_map('intval', $request->get('ids') ?: [])` |
| **V4** | So sánh chặt: `===` / `!==`, `in_array($v, $list, true)`, `array_search($v, $list, true)`. So chuỗi DB với số → ép kiểu rồi `===`. | `0 == 'abc'` → `false`, `'1e1' == '10'` → `true` |
| **V5** | `empty()` / `!$x` chỉ khi `'0'` và `0` không phải giá trị hợp lệ. Picklist có `'0'`, số lượng / tiền `0` → `=== ''`, `=== null`, `=== 0`. | `if (empty($qty))` bỏ qua số lượng 0 hợp lệ |
| **V6** | Mảng tạo bằng `[]`. List giữ là list, map giữ là map, không trộn. | `$x = ''; $x[] = 1;` → `Error`; `$x = false; $x[] = 1;` → deprecated |
| **V7** | `??` không xử lý `false`: giá trị có thể `false` / `''` / `null` → `?:`; không chắc kiểu → `is_array($x) ? $x : []`. | `count($x ?? [])` với `$x = false` → `TypeError` |

## E. Lỗi và ngoại lệ

| Mã | Rule |
|---|---|
| **E1** | Điểm vào (action, view, API handler, cron, webhook, queue job) PHẢI `try { … } catch (Throwable $e)`: log kèm ngữ cảnh (id, module), trả lỗi đã dịch. KHÔNG đưa `$e->getMessage()` thô cho người dùng. |
| **E2** | `catch (Exception $e)` KHÔNG bắt `TypeError`, `ValueError`, `Error`, `DivisionByZeroError`: ở điểm vào PHẢI là `Throwable`. |
| **E3** | Vòng lặp cron / queue: bắt lỗi **từng phần tử**, một bản ghi lỗi không làm dừng cả lô. |
| **E4** | KHÔNG `catch {}` rỗng; KHÔNG nuốt lỗi rồi trả `false` âm thầm. Sự cố → `throw`; kết quả bình thường (không thấy, rỗng) → `return`. |

## R. Hàm có sẵn của PHP trên 8.3

| Mã | Sai | Trên 8.3 | Đúng |
|---|---|---|---|
| **R1** | `mktime(0, 0, 0, $m, $d, $y)` với `$m = ''`; `str_pad($s, '')`; `round('')` | `TypeError` | truyền số thật: `(int) $m` hoặc kiểm rỗng trước |
| **R2** | `count` / `in_array` / `implode` / `array_filter` / `array_keys` với `null` / `false` / chuỗi | `TypeError` | đảm bảo mảng lúc gán (`?: []`, `is_array`) |
| **R3** | `md5([...])`, hàm chuỗi nhận mảng | `TypeError` | kiểm kiểu trước |
| **R4** | `fread($fp, 0)`, độ dài âm | `ValueError` | `if ($size > 0)` |
| **R5** | `fgetcsv($fp, 0, '')`, delimiter nhiều ký tự | `ValueError` | delimiter đúng 1 ký tự |
| **R6** | `$str['key']`, `$str[] = $x` trên chuỗi | `TypeError` / `Error` | khởi tạo `[]` |
| **R7** | Gọi static một method không static (`Class::method()`) | `Error` | gọi qua instance, hoặc khai báo `static` |
| **R8** | `split()`, `each()`, `create_function()`, `preg_replace` với `/e` | `Error` (đã bị xoá) | `explode()`, `foreach`, closure, `preg_replace_callback()` |
| **R9** | `htmlspecialchars($s)` / `htmlentities($s)` không truyền cờ | `'` thành `&#039;` (đổi từ 8.1) | `htmlspecialchars($s, ENT_QUOTES, 'UTF-8')` |
| **R10** | `mb_convert_encoding($s, 'HTML-ENTITIES', …)` | deprecated 8.2 | `htmlspecialchars` / `mb_encode_numericentity` |
| **R11** | `substr(null, …)`, `explode(',', null)`, `null` vào tham số chuỗi của hàm có sẵn | deprecated | `(string) $x` / `$x ?? ''` |
| **R12** | `declare(strict_types=1)` trong file `modules/` | mọi hàm có sẵn nhận chuỗi DB → `TypeError` | `strict_types` chỉ trong `domains/` |

## S. Cú pháp PHP 8.x cho code mới

| Mã | Rule |
|---|---|
| **S1** | Được dùng: `match` (map giá trị), `??=`, arrow `fn`, `str_contains` / `str_starts_with` / `str_ends_with`, constructor promotion, `readonly`, hằng có kiểu, `enum` (trong `domains/`). |
| **S2** | `?->` chỉ dùng khi **đọc**, KHÔNG dùng cho lệnh có tác dụng phụ (`$model?->save()` im lặng bỏ qua). |
| **S3** | KHÔNG dùng named argument khi gọi hàm vtiger / legacy (tên tham số của chúng không ổn định). |

## Trước khi commit

`php -l <file>` bằng PHP 8.3; gate PHPStan (`rule://phpstan`); chạy thử luồng đã sửa trên PHP 8.3.
