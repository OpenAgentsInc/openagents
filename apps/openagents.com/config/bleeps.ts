export const bleepsSettings = {
  master: {
    volume: 0.5
  },
  bleeps: {
    click: {
      sources: [
        { src: '/sounds/click.mp3', type: 'audio/mpeg' },
        { src: '/sounds/click.webm', type: 'audio/webm' }
      ]
    },
    hover: {
      sources: [
        { src: '/sounds/hover.mp3', type: 'audio/mpeg' },
        { src: '/sounds/hover.webm', type: 'audio/webm' }
      ],
      volume: 0.3
    },
    type: {
      sources: [
        { src: '/sounds/type.mp3', type: 'audio/mpeg' },
        { src: '/sounds/type.webm', type: 'audio/webm' }
      ],
      volume: 0.2
    }
  }
};