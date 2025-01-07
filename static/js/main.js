// Get the canvas element
const canvas = document.querySelector('#bg');

// Initialize the renderer
const renderer = new THREE.WebGLRenderer({
    canvas,
    antialias: true
});
renderer.setPixelRatio(window.devicePixelRatio);
renderer.setSize(window.innerWidth, window.innerHeight);
renderer.setClearColor(0x000000);
renderer.shadowMap.enabled = true;
renderer.toneMapping = THREE.ACESFilmicToneMapping;
renderer.toneMappingExposure = 1.5;

// Initialize scene system
const sceneSystem = new SceneSystem(window.innerWidth, window.innerHeight);

// Handle window resize
window.addEventListener('resize', () => {
    const width = window.innerWidth;
    const height = window.innerHeight;
    
    renderer.setSize(width, height);
    sceneSystem.handleResize(width, height);
});

// Animation loop
function animate() {
    requestAnimationFrame(animate);
    
    const time = Date.now();
    sceneSystem.update(time);
    
    renderer.render(
        sceneSystem.getScene(),
        sceneSystem.getCamera()
    );
}

// Start the animation
animate();