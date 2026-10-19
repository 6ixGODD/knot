"""KNOT ABI v1 binding. Load a trusted, explicitly named native library."""
import ctypes as C
from dataclasses import dataclass
import json
from pathlib import Path

PRODUCT_LIMIT = 4_194_304
OUTPUT_LIMIT = 4_194_304

class _Buffer(C.Structure):
    _fields_ = [('data', C.POINTER(C.c_uint8)), ('len', C.c_size_t)]

class _Result(C.Structure):
    _fields_ = [('status', C.c_int32), ('json', _Buffer)]

class Error(RuntimeError):
    def __init__(self, status, code, message, path='', rule_ids=()):
        super().__init__(message)
        self.status, self.code, self.path = status, code, path
        self.rule_ids = list(rule_ids)

@dataclass
class Result:
    state: dict
    snapshot: dict
    changes: dict
    diagnostics: list
    schema_version: str

def _pairs(pairs):
    out = {}
    for key, value in pairs:
        if key in out:
            raise ValueError('Duplicate JSON key')
        out[key] = value
    return out

def _decode(raw):
    def invalid_constant(value):
        raise ValueError(f'Non-JSON number: {value}')
    return json.loads(raw.decode('utf-8'), object_pairs_hook=_pairs, parse_constant=invalid_constant)

def _encode(value, limit):
    raw = json.dumps(value, ensure_ascii=False, separators=(',', ':'), allow_nan=False).encode('utf-8')
    if len(raw) > limit:
        raise Error(1011, 'RESOURCE_LIMIT', 'Input exceeds byte limit')
    return raw

def _input(raw):
    # A non-NULL allocation is retained even for empty input; length excludes NUL.
    return (C.c_uint8 * max(1, len(raw))).from_buffer_copy(raw or b'\0')

class Model:
    def __init__(self, definition, *, library):
        self._handle = C.c_void_p()
        path = Path(library)
        if not path.is_absolute():
            raise ValueError('library must be an explicit absolute path')
        self._lib = C.CDLL(str(path.resolve(strict=True)))
        u8 = C.POINTER(C.c_uint8)
        signatures = {
            'knot_abi_version': ([], C.c_uint32),
            'knot_model_create': ([u8, C.c_size_t, C.POINTER(C.c_void_p)], _Result),
            'knot_initial': ([C.c_void_p], _Result),
            'knot_apply': ([C.c_void_p, u8, C.c_size_t, u8, C.c_size_t], _Result),
            'knot_evaluate': ([C.c_void_p, u8, C.c_size_t], _Result),
            'knot_result_free': ([C.POINTER(_Result)], None),
            'knot_model_free': ([C.c_void_p], None),
        }
        for name, (args, result) in signatures.items():
            f = getattr(self._lib, name)
            f.argtypes, f.restype = args, result
        if self._lib.knot_abi_version() != 1:
            raise Error(1999, 'INTERNAL', 'Unsupported native ABI version')
        raw = definition.encode('utf-8') if isinstance(definition, str) else bytes(definition)
        if len(raw) > PRODUCT_LIMIT:
            raise Error(1011, 'RESOURCE_LIMIT', 'Product exceeds byte limit')
        buffer = _input(raw)
        try:
            self.info = self._consume(self._lib.knot_model_create(buffer, len(raw), C.byref(self._handle)))
            if not self._handle or set(self.info) != {'schemaVersion', 'productId', 'productVersion', 'modelDigest', 'diagnostics'}:
                raise Error(1003, 'BAD_SCHEMA', 'Invalid model-create result')
        except BaseException:
            self.close()
            raise

    @classmethod
    def from_file(cls, path, *, library):
        with open(path, 'rb') as file:
            raw = file.read(PRODUCT_LIMIT + 1)
        return cls(raw, library=library)

    def _consume(self, result):
        try:
            if not result.json.data or not result.json.len:
                if result.status == 1999 and not result.json.data and result.json.len == 0:
                    raise Error(1999, 'INTERNAL', 'Native internal failure (no JSON)')
                raise Error(1003, 'BAD_SCHEMA', 'Native result contains no JSON')
            if result.json.len > OUTPUT_LIMIT:
                raise Error(1011, 'RESOURCE_LIMIT', 'Native output exceeds byte limit')
            raw = C.string_at(result.json.data, result.json.len)
            try:
                value = _decode(raw)
                if not isinstance(value, dict) or value.get('schemaVersion') != '1.0':
                    raise ValueError('Invalid response version')
                if result.status:
                    error = value['error']
                    raise Error(result.status, error['code'], error['message'], error['path'], error['ruleIds'])
                return value
            except (ValueError, KeyError, TypeError, RecursionError) as exc:
                raise Error(1002, 'BAD_JSON', 'Malformed native response') from exc
        finally:
            self._lib.knot_result_free(C.byref(result))

    def _open(self):
        if not self._handle:
            raise Error(1001, 'NULL_ARGUMENT', 'Model is closed')

    def _transition(self, raw):
        value = self._consume(raw)
        if set(value) != {'schemaVersion', 'state', 'snapshot', 'changes', 'diagnostics'}:
            raise Error(1003, 'BAD_SCHEMA', 'Invalid transition result')
        return Result(value['state'], value['snapshot'], value['changes'], value['diagnostics'], value['schemaVersion'])

    def initial(self):
        self._open()
        return self._transition(self._lib.knot_initial(self._handle))

    def apply(self, state, event):
        self._open()
        sr, er = _encode(state, 524288), _encode(event, 65536)
        sb, eb = _input(sr), _input(er)
        return self._transition(self._lib.knot_apply(self._handle, sb, len(sr), eb, len(er)))

    def evaluate(self, state):
        self._open()
        raw = _encode(state, 524288)
        buffer = _input(raw)
        return self._transition(self._lib.knot_evaluate(self._handle, buffer, len(raw)))

    def close(self):
        if self._handle:
            self._lib.knot_model_free(self._handle)
            self._handle = C.c_void_p()

    def __enter__(self):
        self._open()
        return self

    def __exit__(self, *_):
        self.close()

    def __copy__(self):
        raise TypeError('Model has unique ownership')

    def __deepcopy__(self, memo):
        raise TypeError('Model has unique ownership')
