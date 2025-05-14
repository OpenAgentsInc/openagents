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
 * Type guard for checking if a value is a record with arbitrary keys
 */
function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null;
}

/**
 * Safe property access - checks if object has a property
 */
function hasProp<K extends string>(obj: unknown, prop: K): obj is { [P in K]: unknown } {
  return isRecord(obj) && prop in obj;
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
    if (hasProp(decoded, 'millisatoshis') && decoded.millisatoshis !== undefined) {
      // Direct millisatoshis property
      amountSat = Math.floor(Number(decoded.millisatoshis) / 1000);
      console.log('Found amount in millisatoshis:', decoded.millisatoshis, '-> sats:', amountSat);
    } else if (hasProp(decoded, 'satoshis') && decoded.satoshis !== undefined) {
      // Direct satoshis property (some decoders use this)
      amountSat = Number(decoded.satoshis);
      console.log('Found amount in satoshis:', amountSat);
    } else {
      // Try to get amount from sections
      const amountSection = decoded.sections.find(section => 
        section.name === 'amount');
      
      if (amountSection && hasProp(amountSection, 'value') && amountSection.value !== undefined) {
        // Convert to number, handling both millisats or sats
        const value = Number(amountSection.value);
        if (value > 0) {
          // If it's in millisats (typical for light-bolt11-decoder)
          amountSat = Math.floor(value / 1000);
          console.log('Found amount in sections (millisats):', value, '-> sats:', amountSat);
        }
      }
      
      // Check for amount in section with nested properties
      if (amountSat === 0) {
        for (const section of decoded.sections) {
          // Type guard to check if section has certain properties
          if (hasProp(section, 'value') && isRecord(section.value) && 
              hasProp(section.value, 'amount') && section.value.amount !== undefined) {
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
      section.name === 'description');
    if (descriptionTag && hasProp(descriptionTag, 'value') && descriptionTag.value !== undefined) {
      description = String(descriptionTag.value);
    }
    
    // Extract payment hash
    const paymentHashTag = decoded.sections.find(section => section.name === 'payment_hash');
    let paymentHash: string | undefined = undefined;
    if (paymentHashTag && hasProp(paymentHashTag, 'value') && paymentHashTag.value !== undefined) {
      paymentHash = String(paymentHashTag.value);
    }
    
    // Extract payee node key - Since it's not in standard types, we look for it in raw data
    let payeeNodeKey: string | undefined = undefined;
    
    // Try to find it in various possible locations
    for (const section of decoded.sections) {
      if ((section.name === 'payment_hash' || section.name === 'signature') && 
          hasProp(section, 'value') && typeof section.value === 'string') {
        // Sometimes the signing pubkey is what we want
        payeeNodeKey = section.value;
        break;
      }
    }
    
    // Extract timestamp
    let timestamp: number | undefined = undefined;
    if (hasProp(decoded, 'timestamp') && typeof decoded.timestamp === 'number') {
      timestamp = decoded.timestamp;
    } else {
      // Try to get timestamp from sections
      const timestampSection = decoded.sections.find(section => section.name === 'timestamp');
      if (timestampSection && hasProp(timestampSection, 'value') && timestampSection.value !== undefined) {
        timestamp = Number(timestampSection.value);
      }
    }
    
    // Extract expiry
    const expiryTag = decoded.sections.find(section => section.name === 'expiry');
    let expiry: number | undefined = undefined;
    if (expiryTag && hasProp(expiryTag, 'value') && expiryTag.value !== undefined) {
      expiry = Number(expiryTag.value);
    }
    
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
