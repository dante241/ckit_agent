---
description: "Checklist khi sửa composition root <Domain>.php (vd Social.php) và channel provider (Channel/<Name>/<Name>Channel.php): nơi duy nhất new class thật, factory trả interface, override, đăng ký kênh."
scope: "tool:edit(domains/Social/src/Social.php), tool:write(domains/Social/src/Social.php), tool:edit(domains/**/Channel/**/*Channel.php), tool:write(domains/**/Channel/**/*Channel.php)"
condition: ".*"
---

# Composition root (`domains/<Domain>/src/<Domain>.php`) + channel provider

> Chỉ áp cho file `<Domain>.php` (hiện: `Social.php`) và `Channel/<Name>/<Name>Channel.php`. Tạo domain mới → thêm glob `domains/<D>/src/<D>.php` vào `scope` ở trên.

Được gọi: mọi class của domain, `PearDatabase::getInstance()`, `Settings_Vtiger_Config_Model` (đọc config), `Shared`. `Social::` không xuất hiện trong Service/Repository.

## Checklist

- Factory method `public static`, tên = service camelCase (`conversationListService()`), **khai báo trả kiểu interface** cho port; Service trả class `final` của nó.
- **Site override = factory callable trả về interface** (D-014): `configureOverrides([ScopeGateway::class => fn(callable $makeDefault, string $channel): ScopeGateway => …])`. Factory nhận builder lười của bản mặc định + channel (`''` với service không theo channel) → decorator (`new X($makeDefault())`) hoặc thay hẳn; channel khác trả `$makeDefault()`.
- Port nào cho phép site thay → bọc qua `self::resolve(Interface::class, fn() => new Default(...), $channel)`; `resolve()` từ chối kết quả không `instanceof` interface.
- Service phụ thuộc nền tảng nhận `string $channel` và lấy adapter qua `self::channel($channel)->xxxGateway()`; kênh lạ → `ChannelException`.
- Port không cần Service bọc (vd parser webhook) → factory `public static xxx(): Port` trả thẳng port qua `resolve()` (`Social::feedParser(string $channel)`).
- **Thêm nền tảng**: thư mục `Channel/<Name>/` + class implement contract plugin (`CommentChannel`) + **1 dòng** trong `domains/<Domain>/config/channels.php`; `ChannelRegistry` nạp danh sách, service không biết tên kênh, không sửa service.
- Channel provider: chỉ dựng adapter + khai báo holder; không nghiệp vụ, không SQL; credential đọc lười (khi adapter thật sự gọi).
- Không static state ngoài `$overrides` và memo registry kênh trong 1 request (để config kênh chỉ đọc 1 lần; `configureOverrides()` phải xóa memo); không singleton tự chế; không logic nghiệp vụ trong factory.

Mẫu: `domains/Social/src/Social.php`, `Channel/Facebook/FacebookChannel.php`, `config/channels.php`.
