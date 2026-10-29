#!/usr/bin/env python3
"""The public artifact destination is never replaced by publication."""
import errno,importlib.util,pathlib,tempfile,unittest
spec=importlib.util.spec_from_file_location('owned_builder',pathlib.Path(__file__).with_name('build.py'));builder=importlib.util.module_from_spec(spec);spec.loader.exec_module(builder)
class Publication(unittest.TestCase):
 def test_exclusive_directory_publish(self):
  with tempfile.TemporaryDirectory(prefix='coax-owned-publish-test-') as td:
   root=pathlib.Path(td);source=root/'source';target=root/'target';source.mkdir();(source/'payload').write_text('new');target.mkdir()
   with self.assertRaises(OSError) as error:builder.publish_exclusive(source,target)
   self.assertEqual(error.exception.errno,errno.EEXIST);self.assertTrue(source.is_dir());self.assertEqual(list(target.iterdir()),[])
   (target/'old').write_text('keep')
   with self.assertRaises(OSError) as error:builder.publish_exclusive(source,target)
   self.assertEqual(error.exception.errno,errno.EEXIST);self.assertEqual((target/'old').read_text(),'keep')
   (target/'old').unlink();target.rmdir();builder.publish_exclusive(source,target)
   self.assertFalse(source.exists());self.assertEqual((target/'payload').read_text(),'new')
if __name__=='__main__':unittest.main()
