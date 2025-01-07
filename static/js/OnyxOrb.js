class OnyxOrb extends THREE.Group {
  constructor() {
    super();

    // Create gem geometry with crystalline distortion
    const geometry = new THREE.IcosahedronGeometry(1, 2);
    const positions = geometry.attributes.position;
    for (let i = 0; i < positions.count; i++) {
      const x = positions.getX(i);
      const y = positions.getY(i);
      const z = positions.getZ(i);
      const noise = (Math.random() - 0.5) * 0.15;
      positions.setXYZ(i, x + noise, y + noise, z + noise);
    }
    geometry.computeVertexNormals();

    // Main gem material
    const gemMaterial = new THREE.MeshPhysicalMaterial({
      color: 0x000000,
      metalness: 1.0,
      roughness: 0.0,
      reflectivity: 1.0,
      clearcoat: 1.0,
      clearcoatRoughness: 0.0,
      envMapIntensity: 3.0,
      ior: 2.5,
    });

    // Create main gem mesh
    this.gem = new THREE.Mesh(geometry, gemMaterial);
    this.gem.castShadow = true;
    this.gem.receiveShadow = true;
    this.add(this.gem);

    // Add white edges
    const edgeGeometry = new THREE.EdgesGeometry(geometry);
    const edgeMaterial = new THREE.LineBasicMaterial({
      color: 0xffffff,
      transparent: true,
      opacity: 0.4
    });
    this.edges = new THREE.LineSegments(edgeGeometry, edgeMaterial);
    this.add(this.edges);

    // Add inner glow
    const innerGlowMaterial = new THREE.MeshBasicMaterial({
      color: 0xffffff,
      transparent: true,
      opacity: 0.1,
      side: THREE.BackSide
    });
    this.innerGlow = new THREE.Mesh(geometry.clone(), innerGlowMaterial);
    this.innerGlow.scale.multiplyScalar(0.98);
    this.add(this.innerGlow);
  }

  update(time) {
    // Gentle floating motion
    this.position.y = Math.sin(time * 0.001) * 0.1;
    // Slow rotation
    this.rotation.y += 0.005;
  }
}