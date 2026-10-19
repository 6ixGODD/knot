// Package knot owns ABI v1 model handles and result buffers; it contains no rule logic.
package knot

/*
#include <knot.h>
#include <stdlib.h>
*/
import "C"

import (
	"encoding/json"
	"fmt"
	"sync"
	"unsafe"
)

type Error struct {
	Status  int32
	Code    string   `json:"code"`
	Message string   `json:"message"`
	Path    string   `json:"path"`
	RuleIDs []string `json:"ruleIds"`
}

func (e *Error) Error() string { return fmt.Sprintf("%s (%d): %s", e.Code, e.Status, e.Message) }

// Shared ownership state prevents accidental Model value copies from double freeing.
// Calls and Close are serialized; no Go pointer is stored in the native model.
type modelState struct {
	mu     sync.Mutex
	handle *C.knot_model_t
}
type Model struct{ state *modelState }

func consume(r C.knot_result_t) ([]byte, error) {
	defer C.knot_result_free(&r)
	if r.json.data == nil || r.json.len == 0 {
		if r.status == 1999 && r.json.data == nil && r.json.len == 0 {
			return nil, &Error{Status: 1999, Code: "INTERNAL", Message: "native failure without JSON"}
		}
		return nil, &Error{Status: 1003, Code: "BAD_SCHEMA", Message: "native response has no JSON"}
	}
	if r.json.len > 4194304 {
		return nil, &Error{Status: 1011, Code: "RESOURCE_LIMIT", Message: "native output exceeds limit"}
	}
	raw := C.GoBytes(unsafe.Pointer(r.json.data), C.int(r.json.len))
	if !json.Valid(raw) {
		return nil, &Error{Status: 1002, Code: "BAD_JSON", Message: "malformed native JSON"}
	}
	if r.status != 0 {
		var envelope struct {
			SchemaVersion string `json:"schemaVersion"`
			Error         *Error `json:"error"`
		}
		if err := json.Unmarshal(raw, &envelope); err != nil || envelope.SchemaVersion != "1.0" || envelope.Error == nil || envelope.Error.Code == "" {
			return nil, &Error{Status: 1003, Code: "BAD_SCHEMA", Message: "malformed native error"}
		}
		envelope.Error.Status = int32(r.status)
		return nil, envelope.Error
	}
	return raw, nil
}

// Inputs are copied to bounded C allocations, borrowed only during the call.
func input(raw []byte, maximum int) (unsafe.Pointer, error) {
	if len(raw) > maximum {
		return nil, &Error{Status: 1011, Code: "RESOURCE_LIMIT", Message: "input exceeds limit"}
	}
	if len(raw) == 0 {
		return C.CBytes([]byte{0}), nil
	}
	return C.CBytes(raw), nil
}
func Compile(definition []byte) (*Model, []byte, error) {
	if C.knot_abi_version() != 1 {
		return nil, nil, &Error{Status: 1999, Code: "INTERNAL", Message: "unsupported ABI"}
	}
	p, err := input(definition, 4194304)
	if err != nil {
		return nil, nil, err
	}
	defer C.free(p)
	var handle *C.knot_model_t
	raw, err := consume(C.knot_model_create((*C.uint8_t)(p), C.size_t(len(definition)), &handle))
	if err != nil {
		if handle != nil {
			C.knot_model_free(handle)
		}
		return nil, nil, err
	}
	if handle == nil {
		return nil, nil, &Error{Status: 1003, Code: "BAD_SCHEMA", Message: "missing model handle"}
	}
	return &Model{state: &modelState{handle: handle}}, raw, nil
}
func (m *Model) Close() {
	if m == nil || m.state == nil {
		return
	}
	m.state.mu.Lock()
	defer m.state.mu.Unlock()
	if m.state.handle != nil {
		C.knot_model_free(m.state.handle)
		m.state.handle = nil
	}
}
func (m *Model) call(state, event []byte, operation int) ([]byte, error) {
	if m == nil || m.state == nil {
		return nil, &Error{Status: 1001, Code: "NULL_ARGUMENT", Message: "closed model"}
	}
	m.state.mu.Lock()
	defer m.state.mu.Unlock()
	if m.state.handle == nil {
		return nil, &Error{Status: 1001, Code: "NULL_ARGUMENT", Message: "closed model"}
	}
	if operation == 0 {
		return consume(C.knot_initial(m.state.handle))
	}
	s, err := input(state, 524288)
	if err != nil {
		return nil, err
	}
	defer C.free(s)
	if operation == 1 {
		return consume(C.knot_evaluate(m.state.handle, (*C.uint8_t)(s), C.size_t(len(state))))
	}
	e, err := input(event, 65536)
	if err != nil {
		return nil, err
	}
	defer C.free(e)
	return consume(C.knot_apply(m.state.handle, (*C.uint8_t)(s), C.size_t(len(state)), (*C.uint8_t)(e), C.size_t(len(event))))
}
func (m *Model) Initial() ([]byte, error)                  { return m.call(nil, nil, 0) }
func (m *Model) Evaluate(state []byte) ([]byte, error)     { return m.call(state, nil, 1) }
func (m *Model) Apply(state, event []byte) ([]byte, error) { return m.call(state, event, 2) }
