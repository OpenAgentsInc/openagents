"""Unit tests for Gym finalize helpers. No network."""

import unittest

from post_gym_run import catalog_model_from_config, model_from_coder_log


class CatalogModelTest(unittest.TestCase):


    def test_catalog_model_strips_the_harbor_provider_prefix(self):
        config = {
            "agents": [
                {
                    "import_path": "adapters.openagents_coder:OpenAgentsCoder",
                    "model_name": "zai/glm-5.3-flash",
                }
            ]
        }
        self.assertEqual(catalog_model_from_config(config), "glm-5.3-flash")

    def test_catalog_model_missing_is_none(self):
        self.assertIsNone(catalog_model_from_config({}))
        self.assertIsNone(catalog_model_from_config({"agents": []}))

    def test_model_from_coder_log(self):
        text = "Ready\nModel: glm-5.3-flash\nUsage: 1 prompt + 1 completion\n"
        self.assertEqual(model_from_coder_log(text), "glm-5.3-flash")
        self.assertIsNone(model_from_coder_log("no model line"))


if __name__ == "__main__":
    unittest.main()
