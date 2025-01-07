class ViewSystem {
  constructor(width, height) {
    this.group = new THREE.Group();

    // Use even narrower FOV to reduce perspective distortion
    this.camera = new THREE.PerspectiveCamera(55, width / height, 0.1, 1000);
    this.group.add(this.camera);

    this.orb = new OnyxOrb();
    this.group.add(this.orb);

    // Set initial camera position for single view
    this.camera.position.set(0, 1.5, 3.5);
    this.camera.lookAt(0, 0, 0);
    this.orb.position.set(0, 0, 0);
    this.orb.scale.set(1, 1, 1);
  }

  update(time) {
    // Update orb animations
    this.orb.update(time);
  }

  handleResize(width, height) {
    this.camera.aspect = width / height;
    this.camera.updateProjectionMatrix();
  }

  getCamera() {
    return this.camera;
  }

  getGroup() {
    return this.group;
  }
}