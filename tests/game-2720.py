"""Run against the installed executable and the retained 2.7.1.0 fixture."""
from pathlib import Path
import hashlib
import re
import sys
import unittest

import capstone
import pefile

ROOT = Path(__file__).resolve().parents[1]
WORK = ROOT.parents[1] / 'work'
GAME = Path(sys.argv.pop(1)) if len(sys.argv) > 1 else Path('F:/SteamLibrary/steamapps/common/ELDEN RING/Game/eldenring.exe')
PAIRS = [
    (0x255900, 0x255900, 0x7f), (0x24d1e0, 0x24d1e0, 0x19c),
    (0x6871c0, 0x687670, 0xcd), (0x561400, 0x5618b0, 0x56d),
    (0x249160, 0x249160, 0x1ac), (0xd56a10, 0xd58100, 0x728),
    (0x785200, 0x7856b0, 0x24f), (0x395290, 0x395290, 0x77),
    (0x3a2cb0, 0x3a2cb0, 0x13c), (0x4b8050, 0x4b8130, 0x10d),
    (0x4b6e80, 0x4b6f60, 0x51), (0x4b72f0, 0x4b73d0, 0x120),
    (0x4b85b0, 0x4b8690, 0xa1),
]


class Compatibility(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.old = pefile.PE(str(WORK / 'package-v88-game-fixture/eldenring.exe'), fast_load=True)
        cls.new = pefile.PE(str(GAME), fast_load=True)

    def test_native_interfaces_retain_instruction_layout(self):
        dis = capstone.Cs(capstone.CS_ARCH_X86, capstone.CS_MODE_64)
        dis.detail = True

        def normalized(pe, address, size):
            result = []
            consumed = 0
            for ins in dis.disasm(pe.get_data(address, size), address):
                raw = bytearray(ins.bytes)
                consumed += len(raw)
                if ins.disp_size and any(op.type == capstone.x86.X86_OP_MEM and op.mem.base == capstone.x86.X86_REG_RIP for op in ins.operands):
                    raw[ins.disp_offset:ins.disp_offset + ins.disp_size] = bytes(ins.disp_size)
                if ins.imm_size and (ins.group(capstone.CS_GRP_CALL) or ins.group(capstone.CS_GRP_JUMP)):
                    raw[ins.imm_offset:ins.imm_offset + ins.imm_size] = bytes(ins.imm_size)
                result.append(bytes(raw))
            self.assertEqual(consumed, size)
            return result

        for old, new, size in PAIRS:
            with self.subTest(interface=hex(old)):
                self.assertEqual(normalized(self.old, old, size), normalized(self.new, new, size))

    def test_all_checked_interfaces_use_new_addresses_and_matching_signatures(self):
        valid = {new for _, new, _ in PAIRS} | {0x78589b}
        found = set()
        for source in (ROOT / 'native/src').glob('*.rs'):
            text = source.read_text(encoding='utf-8')
            for match in re.finditer(r'checked_function\(\s*(0x[0-9a-f]+)', text):
                self.assertIn(int(match[1], 16), valid, f'{source.name}: {match[1]}')
            text = re.sub(r'&expected\b', lambda _: '&[' + re.search(r'let expected = \[([^]]+)\]', text)[1] + ']', text) if '&expected' in text else text
            for match in re.finditer(r'checked_function\(\s*(0x[0-9a-f]+)\s*,\s*&\[([^]]+)\]', text):
                address = int(match[1], 16)
                expected = bytes(int(v.strip(), 0) for v in match[2].split(',') if v.strip())
                with self.subTest(file=source.name, address=match[1]):
                    self.assertIn(address, valid)
                    self.assertEqual(self.new.get_data(address, len(expected)), expected)
                found.add(address)
        self.assertTrue({0x687670, 0x5618b0, 0xd58100, 0x78589b, 0x4b8130, 0x4b6f60, 0x4b73d0, 0x4b8690} <= found)

    def test_executable_matches_audited_build(self):
        self.assertEqual(hashlib.sha256(GAME.read_bytes()).hexdigest(), 'f9118fea2cfd8fa3901e8060bfadb1552cb2471771ded8a5b662c4e298ffb328')

    def test_world_constructor_uses_new_creator_and_camera_fields(self):
        self.assertEqual(self.old.get_data(0x505abe, 7), bytes.fromhex('48 89 83 48 e6 01 00'))
        self.assertEqual(self.new.get_data(0x505bee, 7), bytes.fromhex('48 89 83 58 e6 01 00'))
        self.assertEqual(self.new.get_data(0x5050c5, 7), bytes.fromhex('4c 89 ab f0 ec 01 00'))


if __name__ == '__main__':
    unittest.main()
