import copy
import ctypes as C
import json
import os
from pathlib import Path
import unittest
from knot import Model, Error, _Buffer, _Result

LIB = os.environ['KNOT_TEST_NATIVE_LIB']
CONTRACTS = Path(__file__).resolve().parents[2] / 'contracts'

class NativeBoundary(unittest.TestCase):
    def setUp(self):
        self.lib = C.CDLL(LIB)
        for name in ('knot_test_reset',):
            getattr(self.lib, name).argtypes = []
            getattr(self.lib, name).restype = None
        self.lib.knot_test_response.argtypes = [C.c_char_p, C.c_size_t, C.c_int32]
        self.lib.knot_test_response.restype = None
        self.lib.knot_test_create_response.argtypes = [C.c_int]
        self.lib.knot_test_create_response.restype = None
        for name in ('knot_test_results_freed', 'knot_test_models_freed', 'knot_test_live_models', 'knot_test_live_buffers'):
            getattr(self.lib, name).argtypes = []
            getattr(self.lib, name).restype = C.c_int
        self.lib.knot_test_layout.argtypes = [C.c_int]
        self.lib.knot_test_layout.restype = C.c_size_t
        for name in ('knot_test_last_state', 'knot_test_last_event'):
            getattr(self.lib, name).argtypes = []
            getattr(self.lib, name).restype = C.c_char_p
        self.lib.knot_test_reset()

    def tearDown(self):
        self.assertEqual(self.lib.knot_test_live_models(), 0)
        self.assertEqual(self.lib.knot_test_live_buffers(), 0)

    def respond(self, value, status=0):
        raw = value.encode() if isinstance(value, str) else value
        self.lib.knot_test_response(raw, len(raw) if raw else 0, status)

    def model(self):
        return Model.from_file(CONTRACTS / 'examples/speaker.json', library=LIB)

    def test_native_layout(self):
        self.assertEqual(C.sizeof(_Buffer), self.lib.knot_test_layout(0))
        self.assertEqual(C.sizeof(_Result), self.lib.knot_test_layout(1))
        self.assertEqual(_Result.json.offset, self.lib.knot_test_layout(2))

    def test_lifetime_and_utf8(self):
        with self.model() as m:
            r = m.initial()
            r.state['answers']['中文'] = '9007199254740990.123456789'
            m.apply(r.state, {'schemaVersion':'1.0','type':'set_answer','questionId':'中文','value':'值','expectedRevision':5})
            self.assertEqual(json.loads(self.lib.knot_test_last_state())['answers']['中文'], '9007199254740990.123456789')
            self.assertEqual(json.loads(self.lib.knot_test_last_event())['value'], '值')
            m.evaluate(r.state)
            with self.assertRaises(TypeError): copy.copy(m)
        m.close()
        self.assertEqual(self.lib.knot_test_models_freed(), 1)
        self.assertEqual(self.lib.knot_test_results_freed(), 4)
        with self.assertRaises(Error): m.initial()

    def test_bad_outputs_are_freed(self):
        with self.model() as m:
            for value in ('{', '{}', '{"schemaVersion":"1.0","schemaVersion":"1.0"}', b'\xff', 'null'):
                before = self.lib.knot_test_results_freed()
                self.respond(value)
                with self.assertRaises(Error): m.initial()
                self.assertEqual(self.lib.knot_test_results_freed(), before + 1)
            self.respond(None, 1999)
            with self.assertRaises(Error) as caught: m.initial()
            self.assertEqual(caught.exception.code, 'INTERNAL')

    def test_error_metadata(self):
        with self.model() as m:
            self.respond(json.dumps({'schemaVersion':'1.0','error':{'code':'INVALID_OPTION','message':'disabled','path':'/value','ruleIds':['rule:/q'],'details':{}}}),1007)
            with self.assertRaises(Error) as caught: m.initial()
            self.assertEqual((caught.exception.status,caught.exception.path,caught.exception.rule_ids),(1007,'/value',['rule:/q']))

    def test_create_parse_failure_frees_model(self):
        self.respond('{')
        self.lib.knot_test_create_response(1)
        with self.assertRaises(Error): self.model()
        self.assertEqual(self.lib.knot_test_models_freed(), 1)
        self.assertEqual(self.lib.knot_test_results_freed(), 1)

    def test_explicit_path_and_limit(self):
        with self.assertRaises(ValueError): Model('{}', library='knot.dll')
        with self.assertRaises(Error): Model(b' ' * 4194305, library=LIB)

if __name__ == '__main__': unittest.main()
