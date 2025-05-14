import { decode } from 'light-bolt11-decoder';

/**
 * Decoded Lightning invoice information
 */
export interface DecodedLnInvoiceInfo {
  amountSat: number;
  description: string;
  paymentRequest: string;
  timestamp?: number;
  expiry?: number;
  paymentHash?: string;
  payeeNodeKey?: string;
}

/**
 * Decodes a BOLT11 Lightning invoice using light-bolt11-decoder
 * 
 * @param invoiceString - The BOLT11 formatted Lightning invoice string
 * @returns DecodedLnInvoiceInfo object with parsed details or null if invalid
 */
export function decodeLightningInvoice(invoiceString: string): DecodedLnInvoiceInfo | null {
  try {
    // Clean the invoice
    const cleanedInvoice = invoiceString.trim().replace(/\s+/g, '').replace(/[\u0000-\u001F\u007F-\u009F]/g, '');
    
    // Validate lightning invoice prefix
    const lowerInvoice = cleanedInvoice.toLowerCase();
    if (!lowerInvoice.startsWith('lnbc') && !lowerInvoice.startsWith('lntb') && !lowerInvoice.startsWith('lnbcrt')) {
      console.error('Invalid Lightning invoice prefix');
      return null;
    }
    
    // Decode the invoice using light-bolt11-decoder
    const decoded = decode(cleanedInvoice);
    console.log('Decoded invoice raw data:', JSON.stringify(decoded, null, 2));
    
    // Extract amount in satoshis
    let amountSat = 0;
    
    // The amount can be in different places depending on the invoice structure
    if (decoded.millisatoshis) {
      // Direct millisatoshis property
      amountSat = Math.floor(Number(decoded.millisatoshis) / 1000);
      console.log('Found amount in millisatoshis:', decoded.millisatoshis, '-> sats:', amountSat);
    } else if (decoded.satoshis) {
      // Direct satoshis property (some decoders use this)
      amountSat = Number(decoded.satoshis);
      console.log('Found amount in satoshis:', amountSat);
    } else {
      // Try to get amount from sections
      const amountSection = decoded.sections.find(section => 
        section.name === 'amount' || section.name === 'value');
      
      if (amountSection?.value) {
        // Convert to number, handling both millisats or sats
        const value = Number(amountSection.value);
        if (amountSection.name === 'amount' && value > 0) {
          // If it's in millisats (typical for light-bolt11-decoder)
          amountSat = Math.floor(value / 1000);
          console.log('Found amount in sections (millisats):', value, '-> sats:', amountSat);
        } else {
          // If it's already in sats
          amountSat = value;
          console.log('Found amount in sections (sats):', amountSat);
        }
      }
      
      // Check for amount in section.value.amount (some libraries nest it here)
      if (amountSat === 0) {
        for (const section of decoded.sections) {
          if (section.value && typeof section.value === 'object' && section.value.amount) {
            amountSat = Number(section.value.amount);
            console.log('Found amount in nested section value:', amountSat);
            break;
          }
        }
      }
    }
    
    // Look for human readable part encoding of amount in the prefix (lnbc2... means 2 sats)
    if (amountSat === 0) {
      const match = lowerInvoice.match(/^ln[a-z]+(\d+)([munp]?)(.*)$/);
      if (match) {
        const value = match[1];
        const multiplier = match[2];
        
        let extractedAmount = Number(value);
        
        // Apply appropriate multiplier
        if (multiplier === 'p') {
          // pico: *0.000000000001 (divide by 1 trillion)
          extractedAmount = extractedAmount / 1000000000000;
        } else if (multiplier === 'n') {
          // nano: *0.000000001 (divide by 1 billion)
          extractedAmount = extractedAmount / 1000000000;
        } else if (multiplier === 'u') {
          // micro: *0.000001 (divide by 1 million)
          extractedAmount = extractedAmount / 1000000;
        } else if (multiplier === 'm') {
          // milli: *0.001 (divide by 1000)
          extractedAmount = extractedAmount / 1000;
        }
        
        // Convert BTC to satoshis
        amountSat = Math.round(extractedAmount * 100000000);
        console.log('Extracted amount from invoice prefix:', amountSat);
      }
    }
    
    // Extract description
    let description = 'No description';
    const descriptionTag = decoded.sections.find(section => 
      section.name === 'description' || section.name === 'purpose_commit');
    if (descriptionTag?.value) {
      description = String(descriptionTag.value);
    }
    
    // Extract payment hash
    const paymentHashTag = decoded.sections.find(section => section.name === 'payment_hash');
    const paymentHash = paymentHashTag?.value ? String(paymentHashTag.value) : undefined;
    
    // Extract payee node key
    const payeeNodeTag = decoded.sections.find(section => section.name === 'payee_node_key');
    const payeeNodeKey = payeeNodeTag?.value ? String(payeeNodeTag.value) : undefined;
    
    // Extract timestamp
    const timestamp = decoded.timestamp;
    
    // Extract expiry
    const expiryTag = decoded.sections.find(section => section.name === 'expiry');
    const expiry = expiryTag?.value ? Number(expiryTag.value) : undefined;
    
    return {
      amountSat,
      description,
      paymentRequest: cleanedInvoice,
      timestamp,
      expiry,
      paymentHash,
      payeeNodeKey,
    };
  } catch (error) {
    console.error('Error decoding Lightning invoice:', error);
    return null;
  }
}
