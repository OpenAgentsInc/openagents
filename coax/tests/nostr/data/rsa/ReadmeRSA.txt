Example test files for FIPS 186-3 RSA
Updated May 12, 2015 to add examples for the truncated SHAs and to remove mod 1024 and SigGen with SHA1. 

1. The files with extension '.rsp' are response files in the proper format for CAVS validation.

2. The file SigGenRSA_186-3.txt contains values for X9.31RSA signature generation with the additional value d added to the file for testing purposes.

3.  The file SigGen15_186-3.txt contains values for RSA PKCS#1 Ver 1.5 signature generation with the additional value d added to the file for testing purposes.

4.  The file SigGenPSS_186-3.txt contains values for RSA PKCS#1 RSASSA-PSS signature generation with the additional value d and Saltvalue added to the file for testing purposes.

5.  The files for Signature Verification include both a file for the Truncated SHAs and a file that contains all the other valid SHAs. 

6.  The file KeyGen_186-3.rsp contains values for X9.31 RSA Key Generation all methods and all regular SHAs.  The file KeyGen_186-3_TruncatedSHAs.rsp contains values for X9.31 RSA Key Generation where the truncated SHAs are used.  This includes Appendix B.3.2, Appendix B.3.4, and Appendix B.3.5.

7.  The file KeyGen_RandomProbablyPrime3_3 contains 10 sets of data for each mod/M-R Table combination.  The example given here does not provide 10 different values for each set.  It is only an example of a couple valid sets of values that can be used to assure the IUT generates the correct answer given known input.  A real file from the IUT will contain 10 unique sets of values for each combination.