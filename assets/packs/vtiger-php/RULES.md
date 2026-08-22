# Sticky Rules (always-apply — mọi turn)

> MUST tối thiểu, chắt từ các rule quy-trình. Chi tiết đọc `rule://<name>` (một hop).

## Code / chất lượng — `rule://development-rules`
- Implement hành vi THẬT. KHÔNG fake data, mock, hay shortcut tạm chỉ để qua check.
- KHÔNG giấu lỗi test / lint / type / build / syntax.
- KHÔNG commit secret / `.env` / token / private key / DB credential / personal data.
- Giữ scope đúng yêu cầu; giữ nguyên public contract trừ khi user đã chấp nhận đổi scope.

## Review / quyết định — `rule://review-audit-self-decision`
- KHÔNG tự đảo quyết định đã verify (source/test/empirical) hay quyết định user đã chốt (threshold, thư viện, scope, schema, pricing, UX…). Nếu audit đề xuất đảo → trình: quyết định gốc + concern + tradeoff + options → CHỜ user.
- KHÔNG nhét plan ID / phase number / audit label / finding code (`EP-NNN`, `WR-NN`) hay ticket number vào code comment / migration / test / commit. Giải thích invariant trực tiếp.

## Delegate subagent — `rule://orchestration-protocol`
- Prompt subagent phải đủ: task · files được đọc · files được sửa · acceptance criteria · constraints · work-context path.
- KHÔNG parallel edit cùng file / migration sequence / shared config. Không pass full history.

## Docs — `rule://documentation-management`
- Update docs CHỈ khi đổi hành vi user-visible / setup / architecture / public contract / security. Đọc doc cũ trước, verify date/link/claim sau.

## Anti-patterns — `rule://error-patterns`
- Trước khi code/review, check catalog `EP-NNN` để tránh tái phát lỗi đã biết.
