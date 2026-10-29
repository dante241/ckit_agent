---
description: "PHP 8.3 rules có mã (C class, F input/output hàm, P kiểm tra trước khi gọi, V biến, E lỗi, R builtin 8.3, S cú pháp 8.x). BẮT BUỘC khi code PHP; là tiêu chí review (trích mã rule trong finding). Bản sao nguyên văn docs/php83-coding-rules.md."
scope: "tool:edit(**/*.php), tool:write(**/*.php), tool:edit(**/*.inc), tool:write(**/*.inc), tool:edit(**/*.service), tool:write(**/*.service)"
condition: ".*"
---

<!-- Verbatim copy of docs/php83-coding-rules.md (team source, tracked in git). Edit the doc, then regenerate this file. -->

# Rules: viết code PHP 8.3

**Cập nhật:** 2026-10-28 · Runtime: **PHP 8.3** (D-019) · Phạm vi: code **mới** và **đoạn code được sửa** trong `modules/`, `include/`, `includes/`, `api/`, `cron/`, `domains/`, `integrations/`. Không sửa format hàng loạt code cũ.

- **PHẢI** / **KHÔNG** = bắt buộc. Vi phạm là finding khi review: ghi mã rule (vd `F7`) trong comment review.
- Style chung (header, K&R, đặt tên, comment): `docs/code-standards.md`. Gate PHPStan: `docs/phpstan.md`.
- Mọi hành vi ghi ở cột "Trên 8.3" đã chạy thử 7.4 so với 8.3. Trên 7.4 phần lớn chỉ là cảnh báo; **trên 8.3 thì request chết** (`TypeError` / `ValueError` / `Error`).
- Agent dùng cùng file này, cùng mã rule (`.omp/rules/php83.md`).

---

## C. Khai báo class

| Mã | Rule |
|---|---|
| **C1** | Thứ tự PHẢI là: hằng → thuộc tính static → thuộc tính → `__construct` → hàm static tạo instance (`getInstance`, `fromRow`) → `public` → `protected` → `private`. |
| **C2** | Hằng PHẢI có phạm vi + kiểu: `public const string STATUS_ACTIVE = 'Active';`. Giá trị cứng (trạng thái, loại, giới hạn) PHẢI là hằng, KHÔNG lặp chuỗi ở nhiều chỗ. |
| **C3** | Thuộc tính PHẢI có phạm vi + kiểu + giá trị mặc định, trừ khi constructor luôn gán. KHÔNG `var $x;`, KHÔNG thuộc tính thiếu kiểu trong class mới. |
| **C4** | Thuộc tính `array` PHẢI có `@var`: `list<int>`, `array<int, string>`, `array{id: int, name: string}`. |
| **C5** | Thuộc tính chỉ gán trong constructor → `readonly`. Class nhận phụ thuộc / giá trị đơn giản → constructor promotion. |
| **C6** | KHÔNG tạo thuộc tính động (`$this->x = …` khi chưa khai báo). Class legacy đang dựa vào thuộc tính động thì giữ `#[\AllowDynamicProperties]`, KHÔNG gỡ. |
| **C7** | Model vtiger (`Vtiger_Base_Model`, `Vtiger_Record_Model`): dữ liệu record đọc/ghi qua `get()` / `set()`. KHÔNG khai báo thuộc tính trùng tên field; thuộc tính chỉ dành cho state riêng của class. |
| **C8** | Thuộc tính `static` chỉ dùng làm cache **có khoá**, có kiểu: `private static array $cache = [];`. KHÔNG singleton bỏ qua tham số (instance của user 7 bị trả cho user 9). |
| **C9** | Helper / service / utils mới không có lớp con → `final class`. KHÔNG thêm hàm global mới vào `include/utils/*`; đặt thành method static trong helper của module. |
| **C10** | Override PHẢI tương thích chữ ký class cha: cùng số tham số, tham số cha có default thì con cũng PHẢI có default. Bỏ default (`f($a, $b)` override `f($a, $b = 1)`) → 7.4 chỉ cảnh báo, **8.3 fatal ngay khi nạp class**. |

| Trên 8.3 (đã đo) | Kết quả |
|---|---|
| Đọc thuộc tính có kiểu chưa gán | `Error: must not be accessed before initialization` |
| Gán `''` hoặc `null` vào thuộc tính `int` | `TypeError` (`'12'` thì tự đổi thành 12) |
| Gán lại thuộc tính `readonly` | `Error: Cannot modify readonly property` |
| Tạo thuộc tính động | deprecated (8.2+) |

```php
final class CPExample_Report_Helper {

	public const string STATUS_ACTIVE = 'Active';
	private const int MAX_ROWS = 100;

	/** @var array<int, self> instance per account */
	private static array $instances = [];

	/** @var list<int> */
	private array $employeeIds = [];

	private ?Vtiger_Record_Model $recordModel = null;

	public function __construct(
		private readonly int $accountId,
		private readonly string $fromDate,
		private readonly ?string $toDate = null
	) {
	}

	public static function getInstance(int $accountId, string $fromDate): self {
		return self::$instances[$accountId] ??= new self($accountId, $fromDate);
	}
}
```

---

## F. Hàm: input / output

| Mã | Rule |
|---|---|
| **F1** | PHẢI ghi phạm vi (`public` / `protected` / `private`), kiểu **mọi** tham số, kiểu trả về (`void` khi không trả). `static` chỉ khi hàm không dùng `$this`. Gate PHPStan tự chặn hàm mới / tham số mới thiếu kiểu (`missingType.parameter`, `missingType.return`). |
| **F2** | Docblock: một dòng mô tả (tiếng Anh); `@param` / `@return` khi kiểu PHP chưa nói đủ (mảng, ý nghĩa của `null`); `@throws` khi hàm ném lỗi. |
| **F3** | Kiểu tham số: `int` / `string` / `bool` / `float` / `array` / class / `?T`. Union chỉ khi thật sự nhận hai kiểu. KHÔNG `mixed` trừ khi chỉ chuyển tiếp. Tham số bắt buộc trước, tuỳ chọn sau. |
| **F4** | **Nơi gọi PHẢI ép kiểu trước khi truyền.** Truyền `''` / `null` vào tham số `int` là `TypeError` trên cả 7.4 lẫn 8.3 (file không `strict_types`); `"12abc"` lỗi trên 8.3. |
| **F5** | KHÔNG thêm kiểu vào hàm cũ khi chưa rà hết nơi gọi (`lsp references` / grep) và lớp con. Một nơi gọi truyền `''` / `null` = lỗi chết mới. Thêm kiểu trả về ở lớp cha mà lớp con override chưa có kiểu → **fatal khi nạp class**; thêm kiểu tham số ở lớp con khi lớp cha chưa có → **fatal**. `: void` mà nơi gọi đang dùng kết quả → hàm đó đang có lỗi logic, sửa nơi gọi trước. |
| **F6** | Hơn 4 tham số → mảng có shape (`@param array{from: string, to: string, ids: list<int>} $filter`) hoặc class giá trị. KHÔNG gán lại tham số (copy sang biến mới). KHÔNG thêm tham số tham chiếu `&$x` mới. |
| **F7** | Giá trị trả về: không tìm thấy → `null` (`?T`); danh sách rỗng → `[]`; có/không → `bool`; sự cố → `throw`. Code mới KHÔNG trả `array\|false`, `string\|false`. |
| **F8** | **Mọi nhánh** PHẢI trả đúng kiểu khai báo (hàm `: string` mà `return null` → `TypeError`). Tham số / giá trị trả về kiểu `array` PHẢI ghi nội dung: `@param list<int> $ids`, `@return array{success: bool, message: string}`, `@return list<array{id: int, label: string}>` — gate tự chặn khi thiếu (`missingType.iterableValue`). Ghi shape thật, KHÔNG lách bằng `array<mixed>`: chỉ shape thật mới giúp PHPStan bắt nơi gọi đọc key không tồn tại. |
| **F9** | API handler / hàm facade cũ PHẢI giữ nguyên định dạng response (`['success' => …]`, `setResponse(...)`): chỉ sửa bên trong, KHÔNG đổi contract với client. Helper KHÔNG trả HTML, KHÔNG trả chuỗi đã dịch. |
| **F10** | Kiểm tra đầu vào ở đầu hàm, `return` sớm, không lồng `if` sâu. KHÔNG biến `global` mới (đọc `$adb`, `$current_user`, `$log` sẵn có được; code mới ưu tiên `PearDatabase::getInstance()`, `Users_Record_Model::getCurrentUserModel()`). KHÔNG `@` nuốt lỗi (trừ `@unlink`), `extract`, `compact`, `$$name`, `eval`. |
| **F11** | Không đổi chữ ký hàm public chỉ để "hiện đại hoá". Đổi chữ ký = đổi contract → sửa mọi nơi gọi trong cùng MR. |

```php
/**
 * Worked hours per employee in a date range.
 *
 * @param list<int> $employeeIds
 * @return array<int, float> hours keyed by employee id
 * @throws AppException when the date range is invalid
 */
public static function getWorkedHours(array $employeeIds, string $fromDate, ?string $toDate = null): array {
	if (empty($employeeIds)) return [];
	...
}

// caller (action): cast before calling
$hours = CPTimeSheets_Data_Helper::getWorkedHours(array_map('intval', $request->get('ids') ?: []), (string) $request->get('from'));
```

---

## P. Kiểm tra trước khi gọi

**P0.** Kiểm **một lần** ở chỗ dữ liệu đi vào (đầu action, đầu hàm public), rồi dùng thẳng. KHÔNG lặp cùng một kiểm tra ở nhiều tầng.

| Mã | Trước khi gọi | PHẢI kiểm | Nếu bỏ qua |
|---|---|---|---|
| **P1** | `Vtiger_Record_Model::getInstanceById($id, $module)` | `$id > 0 && isRecordExists($id)`; id từ request thêm `getSalesEntityType($id) === $module` | Exception `LBL_RECORD_NOT_FOUND` / `LBL_RECORD_DELETE` (`data/CRMEntity.php`) |
| **P2** | Đọc / sửa / xoá record | `Users_Privileges_Model::isPermitted($module, 'DetailView' \| 'EditView' \| 'Delete', $id)` | lộ / sửa dữ liệu không có quyền |
| **P3** | `Vtiger_Module_Model::getInstance($name)` | `instanceof Vtiger_Module_Model`; module có thể tắt → `vtlib_isModuleActive($name)` | trả `false` → `Error` |
| **P4** | `->method()` trên giá trị có thể `null` / `false` | `instanceof` hoặc `!== null` | `Error: Call to a member function … on null` |
| **P5** | `fetchByAssoc` / `num_rows` / `query_result` sau `pquery` | `if (!$result)` | SQL lỗi → `false` → `fetchByAssoc(false)` chết |
| **P6** | Dùng `$row` từ `fetchByAssoc()` | `if (!$row)` | hết dòng → `null` |
| **P7** | Key mảng từ request / DB / API / JSON | `$row['key'] ?? $default` | `null` lan xuống dưới |
| **P8** | `json_decode($json, true)` | `is_array($decoded)` | JSON hỏng → `null` |
| **P9** | `strtotime()`, `DateTime::createFromFormat()` | `!== false` | `false->format()` → `Error` |
| **P10** | `fopen`, `fsockopen`, `curl_init`, `file_get_contents` | `!== false` | `fwrite(false)` → `TypeError` |
| **P11** | `fread($fp, $len)` | `$len > 0` (file rỗng, Range ngược) | `ValueError` |
| **P12** | `/`, `%`, `intdiv` | mẫu số `!= 0` | `DivisionByZeroError` |
| **P13** | Toán trên giá trị từ DB / request | đã ép `(int)` / `(float)` | `'' + 1` → `TypeError` |
| **P14** | `count`, `in_array`, `implode`, `array_*` | đã là mảng (`is_array` hoặc `?: []` lúc gán) | `null` / `false` / chuỗi → `TypeError` |
| **P15** | Gọi API bên ngoài | timeout; HTTP status; key trong response tồn tại | treo request; dữ liệu rỗng thành dữ liệu thật |
| **P16** | `new $cls`, `$obj->$method()` | `class_exists`, `method_exists`, `is_callable` | `Error` |
| **P17** | Config (`$GLOBALS[...]`, `vglobal()`, `config_override`) | `?? $default` | site thiếu config → `null` |
| **P18** | User hiện tại trong cron / webhook | `Users_Record_Model::getCurrentUserModel()` có id | cron không có user đăng nhập |

```php
public function process(Vtiger_Request $request) {
	$response = new Vtiger_Response();
	$moduleName = $request->getModule();	// used in catch too: assign before try

	try {
		$recordId = (int) $request->get('record');

		if ($recordId <= 0 || !isRecordExists($recordId) || getSalesEntityType($recordId) !== $moduleName) {
			throw new AppException(vtranslate('LBL_RECORD_NOT_FOUND'));
		}

		if (!Users_Privileges_Model::isPermitted($moduleName, 'EditView', $recordId)) {
			throw new AppException(vtranslate('LBL_PERMISSION_DENIED'));
		}

		$recordModel = Vtiger_Record_Model::getInstanceById($recordId, $moduleName);
		$response->setResult(CPExample_Report_Helper::summary($recordModel));
	}
	catch (Throwable $e) {	// Exception does not catch TypeError / ValueError / Error on PHP 8
		saveLog('CPExample', 'summary failed', ['record' => $request->get('record'), 'error' => $e->getMessage()]);
		$response->setError(vtranslate('LBL_SUMMARY_FAILED', $moduleName));	// key declared in languages/*/CPExample.php
	}

	$response->emit();
}
```

---

## V. Biến

| Mã | Rule | Sai → Đúng |
|---|---|---|
| **V1** | Khởi tạo trước khi dùng, đúng kiểu cuối cùng. KHÔNG định nghĩa trong `if` / vòng lặp rồi dùng ở ngoài. Biến dùng trong `catch` PHẢI gán trước `try`. | `if ($x) { $rows = …; } return $rows;` → `$rows = []; if ($x) { … }` |
| **V2** | Mỗi biến một kiểu; tên nói lên kiểu: `$recordId` (int), `$recordModel` (object), `$rows` / `$recordIds` (danh sách), `$isActive` / `$hasPermission` (bool), `$amount` (float). | `$data` lúc chuỗi lúc mảng |
| **V3** | Ép kiểu **ngay khi gán** dữ liệu từ request / DB / API, sau đó dùng thẳng. Tiền / giờ / % → `(float)`. KHÔNG `(array)` cho danh sách id. | `(array) $request->get('ids')` (`''` → `['']` → id 0 giả) → `array_map('intval', $request->get('ids') ?: [])` |
| **V4** | So sánh chặt: `===` / `!==`, `in_array($v, $list, true)`, `array_search($v, $list, true)`. So chuỗi DB với số → ép kiểu rồi `===`. | `0 == 'abc'`: 7.4 `true`, 8.3 `false` |
| **V5** | `empty()` / `!$x` chỉ khi `'0'` và `0` không phải giá trị hợp lệ. Picklist có `'0'`, số lượng / tiền `0` → `=== ''`, `=== null`, `=== 0`. | `if (empty($qty))` bỏ qua số lượng 0 hợp lệ |
| **V6** | Mảng tạo bằng `[]`. List giữ là list, map giữ là map, không trộn. | `$x = ''; $x[] = 1;` → `Error`; `$x = false; $x[] = 1;` → deprecated |
| **V7** | `??` không xử lý `false`: giá trị có thể `false` / `''` / `null` → `?:`; không chắc kiểu → `is_array($x) ? $x : []`. | `count($x ?? [])` với `$x = false` → `TypeError` |

---

## E. Lỗi và ngoại lệ

| Mã | Rule |
|---|---|
| **E1** | Điểm vào (action, view, API handler, cron, webhook, queue job) PHẢI `try { … } catch (Throwable $e)`: log kèm ngữ cảnh (id, module), trả lỗi đã dịch. KHÔNG đưa `$e->getMessage()` thô cho người dùng. |
| **E2** | `catch (Exception $e)` KHÔNG bắt `TypeError`, `ValueError`, `Error`, `DivisionByZeroError`: ở điểm vào PHẢI là `Throwable`. |
| **E3** | Vòng lặp cron / queue: bắt lỗi **từng phần tử**, một bản ghi lỗi không làm dừng cả lô. |
| **E4** | KHÔNG `catch {}` rỗng; KHÔNG nuốt lỗi rồi trả `false` âm thầm. Sự cố → `throw`; kết quả bình thường (không thấy, rỗng) → `return`. |

---

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

---

## S. Cú pháp PHP 8.x cho code mới

| Mã | Rule |
|---|---|
| **S1** | Được dùng: `match` (map giá trị), `??=`, arrow `fn`, `str_contains` / `str_starts_with` / `str_ends_with`, constructor promotion, `readonly`, hằng có kiểu, `enum` (trong `domains/`). |
| **S2** | `?->` chỉ dùng khi **đọc**, KHÔNG dùng cho lệnh có tác dụng phụ (`$model?->save()` im lặng bỏ qua). |
| **S3** | KHÔNG dùng named argument khi gọi hàm vtiger / legacy (tên tham số của chúng không ổn định). |

---

## Checklist trước khi commit

1. Class: C1-C10 (thuộc tính có phạm vi + kiểu + mặc định; mảng có `@var`; override giữ chữ ký).
2. Hàm mới: F1-F8 (kiểu tham số + trả về; nơi gọi đã ép kiểu; mọi nhánh đúng kiểu; mảng có shape).
3. Đầu vào: V3 (ép kiểu khi gán); P1-P18 (record, quyền, `false` / `null` đã kiểm).
4. Điểm vào: E1-E2 (`catch (Throwable)`).
5. `php -l <file>` bằng PHP 8.3.
6. PHPStan trên các file đã sửa → `[OK] No errors` (`docs/phpstan.md`).
7. Chạy thử luồng đã sửa trên PHP 8.3.

Tự rà nhanh:
```bash
php -l <file>                                                    # PHP 8.3
grep -nE "^\s*(var |function )" <file>                           # C3/F1: thiếu phạm vi (class mới)
grep -nE "count\(\\\$[a-zA-Z_]+ \?\? \[\]\)" <file>               # V7: ?? không bắt false
grep -nE "\b(split|each|create_function)\(|/e['\"]" <file>       # R8: hàm đã xoá
grep -nE "catch \(Exception " <file>                             # E2: điểm vào cần Throwable
grep -nE "getInstanceById\(" <file>                              # P1/P2: rà từng chỗ
grep -nE "\(array\) ?\\\$request" <file>                          # V3: (array) trên request
```
