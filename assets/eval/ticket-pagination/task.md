ESCALATED · SEV-2 · open since 03:41 · shop-api / orders screen

handover from night shift (sorry, didn't get to it):
> 3 customers so far: the first order on page 2 is the same one they just saw at the bottom of page 1
> one guy with 5 orders, 2 per page: footer says "2 pages" but typing page=3 in the URL shows his 5th order
> page=9 on the same account -> 500, log line: "range start index 16 out of range for slice of length 5"
> repro seems to be any account with an odd number of orders? not sure

This repo is the shop crate the HTTP layer calls (HTTP code is NOT here). Fix it. Public function signatures must stay as they are, `cargo test` must stay green.
