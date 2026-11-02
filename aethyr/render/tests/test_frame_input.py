#!/usr/bin/env python3
"""The CPU/Vulkan measurement programs load identical raw observations."""
import subprocess
import tempfile
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent


class FrameInput(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.root = Path(self.temp.name)
        (self.root / "payload.bin").write_bytes(bytes(range(8)))
        (self.root / "render.json").write_text("{}")
        (self.root / "shader.tree").write_text("3 0")
        source = self.root / "probe.c"
        source.write_text('''#include "frame-input.h"
int main(int argc,char **argv){RfiFrame f={0};char error[256];
int ok=argc==3?rfi_load_textures(argv[1],&f,error,sizeof error):rfi_load(argv[1],&f,error,sizeof error);
if(!ok){fprintf(stderr,"%s\\n",error);return 2;}
printf("%lld %zu %zu %zu %zu %llu %zu %s\\n",(long long)f.counts[0],f.uniforms.size,f.shader_count,f.buffer_count,f.texture_count,(unsigned long long)(f.texture_count?f.textures[0].levels[0].row_bytes:0),f.texture_count?f.textures[0].bytes.size:0,f.buffer_count?f.buffers[0].name:"");
if(f.media_text.size&&fwrite(f.media_text.data,1,f.media_text.size,stdout)!=f.media_text.size)return 4;
for(size_t i=0;i<f.media_image_count;i++){RfiMediaImage m=f.media_images[i];printf("%lld %s %lld %lld %lld\\n",(long long)m.kind,m.url,(long long)m.ready,(long long)m.width,(long long)m.height);}
rfi_close(&f);return argc==2||argc==3?0:3;}''')
        self.exe = self.root / "probe"
        subprocess.run(["clang", "-std=c11", "-Wall", "-Wextra", "-Werror", "-I", str(HERE), str(source), "-o", str(self.exe)], check=True, capture_output=True)

    def tearDown(self):
        self.temp.cleanup()

    def config(self, suffix=""):
        return ("15 2 1 6 16 0 0 0 0 0 0 0 0\npayload.bin\n-\n-\n-\nrender.json\n"
                "1\n0 shader.tree\n1\nu.bin payload.bin 8 8 8 0 16 1\n" + suffix)

    def run_config(self, text):
        path = self.root / "frame.config"
        path.write_text(text)
        return subprocess.run([str(self.exe), str(path)], capture_output=True, text=True)

    def test_existing_transport_and_relative_paths(self):
        result = self.run_config(self.config())
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(result.stdout, "15 8 1 1 0 0 0 u.bin\n")

    def test_texture_bytes_and_exact_mip_observations(self):
        result = self.run_config(self.config("textures 1\nsans_atlas rgba8unorm 1 1 payload.bin\n2 1 0 8 8\n"))
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(result.stdout, "15 8 1 1 1 8 8 u.bin\n")

    def test_missing_payload_and_truncation_refuse(self):
        for text in [self.config().replace("payload.bin", "absent.bin", 1), self.config().rsplit(" 1", 1)[0]]:
            result = self.run_config(text)
            self.assertEqual(result.returncode, 2)

    def test_unknown_suffix_and_truncated_texture_refuse(self):
        for suffix in ["unexpected 0\n", "textures 1\nsans_atlas rgba8unorm 1 1 payload.bin\n2 1 0 8\n"]:
            self.assertEqual(self.run_config(self.config(suffix)).returncode, 2)

    def test_numeric_transport_never_saturates_or_reinterprets_a_sign(self):
        for text in [self.config().replace("15 2", "9223372036854775808 2", 1),
                     self.config().replace("8 8 8 0 16 1", "-1 8 8 0 16 1")]:
            self.assertEqual(self.run_config(text).returncode, 2)

    def test_media_text_is_unparsed_owned_bytes_with_or_without_textures(self):
        raw = b"image /first.png\ninvalid\x00line\nvideo /second.webm\n"
        (self.root / "media.txt").write_bytes(raw)
        for suffix in ["media_text media.txt\n", "textures 0\nmedia_text media.txt\n"]:
            result = self.run_config(self.config(suffix))
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertEqual(result.stdout, "15 8 1 1 0 0 0 u.bin\n" + raw.decode())

    def test_missing_duplicate_or_trailing_media_input_refuses(self):
        for suffix in ["media_text\n", "media_text absent.txt\n", "media_text -\nmedia_text -\n", "textures 0\nmedia_text -\nunknown\n"]:
            self.assertEqual(self.run_config(self.config(suffix)).returncode, 2)

    def test_live_texture_descriptor_uses_the_same_raw_suffix_without_producer_counts(self):
        path = self.root / "textures.config"
        path.write_text("textures 1\nsans_atlas rgba8unorm 1 1 payload.bin\n2 1 0 8 8\nmedia_text -\n")
        result = subprocess.run([str(self.exe), str(path), "textures"], capture_output=True, text=True)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(result.stdout, "0 0 0 0 1 8 8 \n")

    def test_media_observations_keep_actual_identity_readiness_and_dimensions(self):
        suffix = "textures 0\nmedia_text -\nmedia_images 2\n1 /actual.png 1 640 480\n2 /actual.webm 0 0 0\n"
        result = self.run_config(self.config(suffix))
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(result.stdout, "15 8 1 1 0 0 0 u.bin\n1 /actual.png 1 640 480\n2 /actual.webm 0 0 0\n")

    def test_truncated_duplicate_or_unrepresentable_media_observation_refuses(self):
        for suffix in ["media_images 1\n1 /actual.png 1 640\n", "media_images 0\nmedia_images 0\n",
                       "media_images 1\n9223372036854775808 /a 1 1 1\n", "media_images -1\n"]:
            self.assertEqual(self.run_config(self.config(suffix)).returncode, 2)


if __name__ == "__main__":
    unittest.main()
