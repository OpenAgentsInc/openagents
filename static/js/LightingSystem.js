class LightingSystem {
  constructor(scene) {
    // Ambient light for base illumination
    const ambientLight = new THREE.AmbientLight(0xffffff, 0.3);
    scene.add(ambientLight);

    // Glow light for orb center
    this.glowLight = new THREE.PointLight(0xffffff, 1.5, 10);
    this.glowLight.position.set(0, 0, 2);
    scene.add(this.glowLight);

    // Pulse light for dynamic effect
    this.pulseLight = new THREE.PointLight(0xffffff, 1.0, 8);
    this.pulseLight.position.set(0, 0, -2);
    scene.add(this.pulseLight);

    // Rim light for edge definition
    this.rimLight = new THREE.SpotLight(0xffffff, 2, 10, Math.PI / 4, 0.5, 1);
    this.rimLight.position.set(3, 2, 0);
    scene.add(this.rimLight);

    // Fill lights for softer shadows
    this.fillLight1 = new THREE.PointLight(0xffffff, 0.5, 10);
    this.fillLight1.position.set(-2, 1, 2);
    scene.add(this.fillLight1);

    this.fillLight2 = new THREE.PointLight(0xffffff, 0.5, 10);
    this.fillLight2.position.set(2, -1, -2);
    scene.add(this.fillLight2);
  }

  update(time) {
    const lightTime = time * 0.002;
    
    // Animate glow and pulse lights
    const intensity = 1.5 + Math.sin(lightTime) * 0.5;
    this.glowLight.intensity = intensity;
    this.pulseLight.intensity = intensity * 0.7;

    // Animate rim light position and intensity
    this.rimLight.position.x = Math.sin(lightTime) * 3;
    this.rimLight.position.z = Math.cos(lightTime) * 3;
    this.rimLight.intensity = 2 + Math.sin(lightTime * 1.5) * 0.5;
  }
}