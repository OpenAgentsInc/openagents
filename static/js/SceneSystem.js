class SceneSystem {
  constructor(width, height) {
    this.scene = new THREE.Scene();
    this.setupFog();
    
    // Initialize systems
    this.lighting = new LightingSystem(this.scene);
    this.viewSystem = new ViewSystem(width, height);
    
    // Add view group to scene
    this.scene.add(this.viewSystem.getGroup());
  }

  setupFog() {
    const sceneColor = 0x000000;
    this.scene.fog = new THREE.Fog(sceneColor, 1, 10000);
  }

  update(time) {
    this.viewSystem.update(time);
    this.lighting.update(time);
  }

  handleResize(width, height) {
    this.viewSystem.handleResize(width, height);
  }

  getScene() {
    return this.scene;
  }

  getCamera() {
    return this.viewSystem.getCamera();
  }
}