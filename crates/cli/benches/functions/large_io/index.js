const OUTPUT = {
  "discountApplicationStrategy": "ALL",
  "discounts": [
    {
      "message": "10OFF4YOU",
      "targets": [ { "productVariant": { "id": "gid://shopify/ProductVariant/29046741890123" } } ],
      "value": { "percentage": { "value": 30 } }
    },
    {
      "message": "10OFF4YOU",
      "targets": [ { "productVariant": { "id": "gid://shopify/ProductVariant/29046773249039" } } ],
      "value": { "percentage": { "value": 20 } }
    },
    {
      "message": "10OFF4YOU",
      "targets": [ { "productVariant": { "id": "gid://shopify/ProductVariant/29046806574195" } } ],
      "value": { "percentage": { "value": 30 } }
    },
    {
      "message": "10OFF4YOU",
      "targets": [ { "productVariant": { "id": "gid://shopify/ProductVariant/29046773284867" } } ],
      "value": { "percentage": { "value": 30 } }
    },
    {
      "message": "10OFF4YOU",
      "targets": [ { "productVariant": { "id": "gid://shopify/ProductVariant/29046673634279" } } ],
      "value": { "percentage": { "value": 30 }
      }
    },
    {
      "message": "10OFF4YOU",
      "targets": [ { "productVariant": { "id": "gid://shopify/ProductVariant/29046701159489" } }
      ],
      "value": { "percentage": { "value": 30 } } },
    {
      "message": "10OFF4YOU",
      "targets": [ { "productVariant": { "id": "gid://shopify/ProductVariant/29046794227619" } } ],
      "value": { "percentage": { "value": 30 } }
    },
    {
      "message": "10OFF4YOU",
      "targets": [ { "productVariant": { "id": "gid://shopify/ProductVariant/29046804791215" } } ],
      "value": { "percentage": { "value": 30 } }
    },
    {
      "message": "10OFF4YOU",
      "targets": [ { "productVariant": { "id": "gid://shopify/ProductVariant/29046693687395" } } ],
      "value": { "percentage": { "value": 30 } } },
    {
      "message": "10OFF4YOU",
      "targets": [ { "productVariant": { "id": "gid://shopify/ProductVariant/29046686741529" } } ],
      "value": { "percentage": { "value": 30 } }
    },
    {
      "message": "10OFF4YOU",
      "targets": [ { "productVariant": { "id": "gid://shopify/ProductVariant/29046687036891" } } ],
      "value": { "percentage": { "value": 30 } }
    },
    {
      "message": "10OFF4YOU",
      "targets": [ { "productVariant": { "id": "gid://shopify/ProductVariant/29046788429683" } } ],
      "value": { "percentage": { "value": 30 } }
    },
    {
      "message": "10OFF4YOU",
      "targets": [ { "productVariant": { "id": "gid://shopify/ProductVariant/29046703125179" } } ],
      "value": { "percentage": { "value": 20 } }
    },
    {
      "message": "10OFF4YOU",
      "targets": [ { "productVariant": { "id": "gid://shopify/ProductVariant/29046743921731" } } ],
      "value": { "percentage": { "value": 20 } }
    },
    {
      "message": "10OFF4YOU",
      "targets": [ { "productVariant": { "id": "gid://shopify/ProductVariant/29046725112905" } } ],
      "value": { "percentage": { "value": 50 } }
    },
    {
      "message": "10OFF4YOU",
      "targets": [ { "productVariant": { "id": "gid://shopify/ProductVariant/29046654268499" } } ],
      "value": { "percentage": { "value": "10" } } },
    {
      "message": "10OFF4YOU",
      "targets": [ { "productVariant": { "id": "gid://shopify/ProductVariant/29046734910532" } } ],
      "value": { "percentage": { "value": "10" } } },
    {
      "message": "10OFF4YOU",
      "targets": [ { "productVariant": { "id": "gid://shopify/ProductVariant/29046736614473" } } ],
      "value": { "percentage": { "value": "10" } } },
    {
      "message": "10OFF4YOU",
      "targets": [ { "productVariant": { "id": "gid://shopify/ProductVariant/29046652597321" } } ],
      "value": { "percentage": { "value": "10" } }
    },
    {
      "message": "10OFF4YOU",
      "targets": [ { "productVariant": { "id": "gid://shopify/ProductVariant/29046723638348" } } ],
      "value": { "percentage": { "value": "10" } }
    },
    {
      "message": "10OFF4YOU",
      "targets": [ { "productVariant": { "id": "gid://shopify/ProductVariant/29046692639810" } } ],
      "value": { "percentage": { "value": "10" } }
    },
    {
      "message": "10OFF4YOU",
      "targets": [ { "productVariant": { "id": "gid://shopify/ProductVariant/29046725407817" } } ],
      "value": { "percentage": { "value": "10" } }
    }
  ]
};

function main(input) {
  // Intentionally not doing any work and returning a large payload
  return OUTPUT;
}

export function readFileSync(fd) {
	let buffer = new Uint8Array(1024);
	let bytesUsed = 0;
	while (true) {
		const bytesRead = Javy.IO.readSync(fd, buffer.subarray(bytesUsed));
		// A negative number of bytes read indicates an error.
		if (bytesRead < 0) {
			// FIXME: Figure out the specific error that occured.
			throw Error("Error while reading from file descriptor");
		}
		// 0 bytes read means we have reached EOF.
		if (bytesRead === 0) {
			return buffer.subarray(0, bytesUsed + bytesRead);
		}

		bytesUsed += bytesRead;
		// If we have filled the buffer, but have not reached EOF yet,
		// double the buffers capacity and continue.
		if (bytesUsed === buffer.length) {
			const nextBuffer = new Uint8Array(buffer.length * 2);
			nextBuffer.set(buffer);
			buffer = nextBuffer;
		}
	}
}

export function writeFileSync(fd, buffer) {
	while (buffer.length > 0) {
		// Try to write the entire buffer.
		const bytesWritten = Javy.IO.writeSync(fd, buffer);
		// A negative number of bytes written indicates an error.
		if (bytesWritten < 0) {
			throw Error("Error while writing to file descriptor");
		}
		// 0 bytes means that the destination cannot accept additional bytes.
		if (bytesWritten === 0) {
			throw Error("Could not write all contents in buffer to file descriptor");
		}
		// Otherwise cut off the bytes from the buffer that
		// were successfully written.
		buffer = buffer.subarray(bytesWritten);
	}
}


// Baseline:
// const buffer = readFileSync(0);
// const inputObj = JSON.parse(new TextDecoder().decode(buffer));
// const outputObj = main(inputObj);
// writeFileSync(1, new TextEncoder().encode(JSON.stringify(outputObj)));

// Javy.JSON:
const buffer = readFileSync(0);
const inputObj = Javy.JSON.parse(new TextDecoder().decode(buffer));
const outputObj = main(inputObj);
writeFileSync(1, new TextEncoder().encode(Javy.JSON.stringify(outputObj)));

// Javy.JSON direct stdin/out:
// const inputObj = Javy.JSON.fromStdin();
// const outputObj = main(inputObj);
// Javy.JSON.toStdout(outputObj );
