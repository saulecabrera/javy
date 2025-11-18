;; This module is a tempate of what the compiler would need to emit
;; for a function like:

;; let secret = 22;
;; function main() {
;;     return secret * secret;
;; }
;; main

;; For which bytecode looks like:

;; func: <eval>
;; -- Closure Vars
;; secret  local  index 0
;; main  local  index 1
;; -- Operators
;; 0x0    PushThis
;; 0x1    IfFalse8 7
;; 0x3    FClosure8 main
;; 0x5    PutVarRef1
;; 0x6    ReturnUndef
;; 0x7    PushI8 22
;; 0x9    PutVarRef0
;; 0xa    GetVarRef1
;; 0xb    Call0
;; 0xc    Drop
;; 0xd    Undefined
;; 0xe    ReturnAsync

;; func: main
;; -- Closure Vars
;; secret    index 0
;; -- Operators
;; 0x0    GetVarRefCheck secret
;; 0x3    GetVarRefCheck secret
;; 0x6    Mul
;; 0x7    Return

(module
  (import "jacrt-link" "functions" (table $t0 2 funcref))

  (import "jacrt" "init" (func $jacrt-init (param i32) (result i32)))
  (import "jacrt" "closure" (func $jacrt-closure (param i32 i32 i32) (result i64)))
  (import "jacrt" "resolve-non-local-var-ref" (func $jacrt-resolve-non-local-var-ref (param i32 i32 i32)))
  (import "jacrt" "put-var-ref" (func $jacrt-put-var-ref (param i32 i32 i64)))
  (import "jacrt" "get-var-ref" (func $jacrt-get-var-ref (param i32 i32) (result i64)))
  (import "jacrt" "get-var-ref-check" (func $jacrt-get-var-ref-check (param i32 i32) (result i64)))
  (import "jacrt" "new-int32" (func $jacrt-new-int32 (param i32 i32) (result i64)))
  (import "jacrt" "call" (func $jacrt-call (param i32 i64) (result i64)))
  (import "jacrt" "mul" (func $jacrt-mul (param i32 i64 i64) (result i64)))

  ;; TODO: Add max number of functions.
  (elem (table $t0) (i32.const 1) funcref (ref.func 10))

  (func (export "_start")
	(local $ctx i32)
	(local $val i64)
	;; Initialize the runtime.
	(call $jacrt-init (i32.const 2))
	(local.tee $ctx)
	(call $jacrt-closure (i32.const 0) (i32.const 1))
	(local.set $val)
	(call $jacrt-resolve-non-local-var-ref (local.get $ctx) (i32.const 1) (i32.const 0))
	(call $jacrt-put-var-ref (local.get $ctx) (i32.const 1) (local.get $val))
	;; TODO: Could create NaN values directly for primitive, non-GC'd values.
	(call $jacrt-new-int32 (local.get $ctx) (i32.const 22))
	(local.set $val)
	(call $jacrt-put-var-ref (local.get $ctx) (i32.const 0) (local.get $val))
	(call $jacrt-get-var-ref (local.get $ctx) (i32.const 1))
	(local.set $val)
	(call $jacrt-call (local.get $ctx) (local.get $val))
	(drop)
  )

  ;; Function index 10
  (func $main (param i32) (result i64)
	(local $r0 i64)
	(local $r1 i64)

	(call $jacrt-get-var-ref-check (local.get 0) (i32.const 0))
	(local.set $r0)
	(call $jacrt-get-var-ref-check (local.get 0) (i32.const 0))
	(local.set $r1)

	(call $jacrt-mul (local.get 0) (local.get $r0) (local.get $r1))
  )
)
