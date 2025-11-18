(module
  (type (func (param i32) (result i64)))
  (table (export "functions") 2 funcref)
  ;; Invoke previously declared functions
  ;; TODO: Param handling
  ;; param 0 = index
  ;; param 1 = context
  (func (export "inv") (param i32 i32) (result i64)
	(call_indirect (type 0) (local.get 1) (local.get 0))
  )	
)
