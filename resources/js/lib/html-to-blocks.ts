/**
 * Converts HTML string to Portable Text blocks
 */
export function parseHtmlToBlocks(html: string) {
  // Create a temporary div to parse HTML
  const div = document.createElement('div')
  div.innerHTML = html

  const blocks: any[] = []

  // Convert each child node to a block
  div.childNodes.forEach((node) => {
    if (node.nodeType === Node.TEXT_NODE) {
      // Handle text nodes
      if (node.textContent?.trim()) {
        blocks.push({
          _type: 'block',
          style: 'normal',
          children: [{ _type: 'span', text: node.textContent }]
        })
      }
    } else if (node.nodeType === Node.ELEMENT_NODE) {
      const element = node as HTMLElement
      const tagName = element.tagName.toLowerCase()
      
      // Create appropriate block based on tag
      switch (tagName) {
        case 'h1':
        case 'h2':
        case 'h3':
        case 'h4':
        case 'h5':
        case 'h6':
          blocks.push({
            _type: 'block',
            style: tagName,
            children: [{ _type: 'span', text: element.textContent || '' }]
          })
          break
          
        case 'p':
          const children: any[] = []
          element.childNodes.forEach((child) => {
            if (child.nodeType === Node.TEXT_NODE) {
              if (child.textContent?.trim()) {
                children.push({ _type: 'span', text: child.textContent })
              }
            } else if (child.nodeType === Node.ELEMENT_NODE) {
              const childElement = child as HTMLElement
              const childTag = childElement.tagName.toLowerCase()
              
              if (childTag === 'a') {
                children.push({
                  _type: 'span',
                  marks: ['link'],
                  text: childElement.textContent || '',
                  markDefs: [{
                    _key: Math.random().toString(36).substr(2, 9),
                    _type: 'link',
                    href: (childElement as HTMLAnchorElement).href
                  }]
                })
              } else if (childTag === 'strong' || childTag === 'b') {
                children.push({
                  _type: 'span',
                  marks: ['strong'],
                  text: childElement.textContent || ''
                })
              } else if (childTag === 'code') {
                children.push({
                  _type: 'span',
                  marks: ['code'],
                  text: childElement.textContent || ''
                })
              } else {
                children.push({ _type: 'span', text: childElement.textContent || '' })
              }
            }
          })
          
          blocks.push({
            _type: 'block',
            style: 'normal',
            children: children
          })
          break
          
        case 'blockquote':
          blocks.push({
            _type: 'block',
            style: 'blockquote',
            children: [{ _type: 'span', text: element.textContent || '' }]
          })
          break
          
        case 'ul':
          Array.from(element.children).forEach((li) => {
            blocks.push({
              _type: 'block',
              style: 'normal',
              listItem: 'bullet',
              children: [{ _type: 'span', text: li.textContent || '' }]
            })
          })
          break
          
        case 'ol':
          Array.from(element.children).forEach((li) => {
            blocks.push({
              _type: 'block',
              style: 'normal',
              listItem: 'number',
              children: [{ _type: 'span', text: li.textContent || '' }]
            })
          })
          break
      }
    }
  })

  return blocks
}