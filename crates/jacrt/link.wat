(module
  (type (func (param i32 i64 i32 i32) (result i64)))
  (table (export "functions") 1 1 funcref)
  ;; Invoke previously declared functions
  ;; param 0 = func index
  ;; param 1 = context
  ;; param 2 = this
  ;; param 3 = argc
  ;; param 3 = argv
  (func (export "inv") (param i32 i32 i64 i32 i32) (result i64)
    (local.get 1)
    (local.get 2)
    (local.get 3)
    (local.get 4)
	(call_indirect (type 0) (local.get 0))
  )
)
